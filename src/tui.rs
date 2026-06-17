use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    client::ApiClient,
    config::{Config, ConfigSecretKey, Secret, update_config_secret},
    endpoints::EndpointIndex,
    error::AppError,
    permissions::{TornPermissionContext, capability_lines},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveField {
    Torn,
    Ffscouter,
}

#[derive(Debug)]
struct ConfigTuiState {
    active: ActiveField,
    torn_input: String,
    ff_input: String,
    torn_replace: bool,
    ff_replace: bool,
    message: String,
    saved: bool,
    capabilities: Vec<String>,
}

impl Default for ConfigTuiState {
    fn default() -> Self {
        Self {
            active: ActiveField::Torn,
            torn_input: String::new(),
            ff_input: String::new(),
            torn_replace: false,
            ff_replace: false,
            message: "Type a new key in the selected field. Enter saves and refreshes permissions; Esc quits.".to_string(),
            saved: false,
            capabilities: vec!["Torn key permissions have not been checked yet.".to_string()],
        }
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self, AppError> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

pub async fn run_config_tui(config: &Config) -> Result<(), AppError> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut state = ConfigTuiState {
        capabilities: load_capabilities(config).await,
        ..ConfigTuiState::default()
    };

    loop {
        terminal.draw(|frame| render_config_tui(frame, config, &state))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if handle_key(config, &mut state, key).await? {
                    break;
                }
            }
        }
    }

    terminal.show_cursor()?;
    Ok(())
}

async fn handle_key(
    config: &Config,
    state: &mut ConfigTuiState,
    key: KeyEvent,
) -> Result<bool, AppError> {
    match key.code {
        KeyCode::Esc => return Ok(true),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(true),
        KeyCode::Tab | KeyCode::BackTab => {
            state.active = match state.active {
                ActiveField::Torn => ActiveField::Ffscouter,
                ActiveField::Ffscouter => ActiveField::Torn,
            };
        }
        KeyCode::Enter => {
            let effective = save_config_tui(config, state)?;
            state.message = "Saved. Refreshing Torn key permissions...".to_string();
            state.capabilities = load_capabilities(&effective).await;
        }
        KeyCode::Backspace => active_input_mut(state).pop().map(|_| ()).unwrap_or(()),
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            active_input_mut(state).clear();
            mark_active_replace(state);
            state.message = "Selected key will be removed on save.".to_string();
        }
        KeyCode::Char(ch) => {
            active_input_mut(state).push(ch);
            mark_active_replace(state);
            state.saved = false;
        }
        _ => {}
    }
    Ok(false)
}

fn active_input_mut(state: &mut ConfigTuiState) -> &mut String {
    match state.active {
        ActiveField::Torn => &mut state.torn_input,
        ActiveField::Ffscouter => &mut state.ff_input,
    }
}

fn mark_active_replace(state: &mut ConfigTuiState) {
    match state.active {
        ActiveField::Torn => state.torn_replace = true,
        ActiveField::Ffscouter => state.ff_replace = true,
    }
}

fn save_config_tui(config: &Config, state: &mut ConfigTuiState) -> Result<Config, AppError> {
    let mut effective = config.clone();
    if state.torn_replace {
        let value = non_empty_secret(&state.torn_input);
        update_config_secret(
            &config.config_path,
            ConfigSecretKey::TornApiKey,
            value.clone(),
        )?;
        effective.torn.api_key = value.and_then(Secret::new);
    }
    if state.ff_replace {
        let value = non_empty_secret(&state.ff_input);
        update_config_secret(
            &config.config_path,
            ConfigSecretKey::FfscouterApiKey,
            value.clone(),
        )?;
        effective.ffscouter.api_key = value.and_then(Secret::new);
    }
    state.saved = true;
    state.message = format!("Saved private config to {}", config.config_path.display());
    Ok(effective)
}

async fn load_capabilities(config: &Config) -> Vec<String> {
    if config.torn.api_key.is_none() {
        return vec!["Torn key: missing; set one to inspect permissions.".to_string()];
    }
    match fetch_capabilities(config).await {
        Ok(lines) => lines,
        Err(error) => vec![format!(
            "Torn key permissions could not be checked: {}",
            error.to_string().replace('\n', " ")
        )],
    }
}

async fn fetch_capabilities(config: &Config) -> Result<Vec<String>, AppError> {
    let client = ApiClient::new(config.clone())?;
    let index = EndpointIndex::load(config.endpoint_index_path.as_deref())?;
    let context = TornPermissionContext::fetch(&client).await?;
    Ok(capability_lines(&context.key_info, &index))
}

fn non_empty_secret(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn render_config_tui(frame: &mut ratatui::Frame<'_>, config: &Config, state: &ConfigTuiState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(8),
            Constraint::Length(7),
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "torn config",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" — private API key setup"),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    render_secret_field(
        frame,
        chunks[1],
        "Torn API key",
        config.torn.api_key.is_some(),
        &state.torn_input,
        state.active == ActiveField::Torn,
        state.torn_replace,
    );
    render_secret_field(
        frame,
        chunks[2],
        "FFScouter API key",
        config.ffscouter.api_key.is_some(),
        &state.ff_input,
        state.active == ActiveField::Ffscouter,
        state.ff_replace,
    );

    let capability_lines = state
        .capabilities
        .iter()
        .take(12)
        .map(|line| Line::from(line.clone()))
        .collect::<Vec<_>>();
    let capabilities = Paragraph::new(capability_lines)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title("Torn key permissions (/key/info)")
                .borders(Borders::ALL),
        );
    frame.render_widget(capabilities, chunks[3]);

    let help = Paragraph::new(vec![
        Line::from(state.message.clone()),
        Line::from(""),
        Line::from("Keys are never displayed. Values are written to config.toml with private permissions where the OS supports it."),
        Line::from(format!("Config path: {}", config.config_path.display())),
        Line::from("Tab switches field · Ctrl+U clears/removes selected key · Enter saves/refreshes permissions · Esc quits"),
    ])
    .wrap(Wrap { trim: true })
    .block(Block::default().title("Help").borders(Borders::ALL));
    frame.render_widget(help, chunks[4]);
}

fn render_secret_field(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    label: &'static str,
    present: bool,
    input: &str,
    active: bool,
    replace: bool,
) {
    let border_style = if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let status = if replace {
        if input.trim().is_empty() {
            "will remove".to_string()
        } else {
            format!("new value: {}", "•".repeat(input.chars().count()))
        }
    } else if present {
        "present (<redacted>)".to_string()
    } else {
        "missing".to_string()
    };
    let paragraph = Paragraph::new(status).block(
        Block::default()
            .title(label)
            .borders(Borders::ALL)
            .border_style(border_style),
    );
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tui_secret_removes_value() {
        assert_eq!(non_empty_secret(""), None);
        assert_eq!(non_empty_secret("  "), None);
        assert_eq!(non_empty_secret(" abc "), Some("abc".to_string()));
    }
}
