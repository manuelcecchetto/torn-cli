use std::process::ExitCode;

use thiserror::Error;

use crate::request::Service;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("missing API key for {service}")]
    MissingApiKey { service: Service },
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("permission denied before request: {0}")]
    PermissionDenied(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("{service} API error{code_text}: {message}", code_text = code.as_ref().map(|c| format!(" {c}")).unwrap_or_default())]
    ApiError {
        service: Service,
        code: Option<String>,
        message: String,
    },
    #[error("JSON error: {0}")]
    Json(String),
    #[error("cache error: {0}")]
    Cache(String),
    #[error("output error: {0}")]
    Output(String),
    #[error("I/O error: {0}")]
    Io(String),
}

impl AppError {
    pub fn network(error: reqwest::Error) -> Self {
        let error = error.without_url();
        Self::Network(error.to_string())
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::HttpStatus { status, .. } => {
                matches!(*status, 408 | 425 | 429 | 500 | 502 | 503 | 504)
            }
            Self::ApiError { service, code, .. } => match service {
                Service::Torn => code
                    .as_deref()
                    .map(|code| matches!(code, "5" | "17" | "24"))
                    .unwrap_or(false),
                Service::Ffscouter => false,
            },
            _ => false,
        }
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Config(_) | Self::MissingApiKey { .. } => ExitCode::from(3),
            Self::PermissionDenied(_) => ExitCode::from(4),
            Self::ApiError { .. } => ExitCode::from(5),
            Self::Network(_) => ExitCode::from(6),
            Self::Cache(_) => ExitCode::from(7),
            Self::Output(_) => ExitCode::from(8),
            _ => ExitCode::from(1),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value.to_string())
    }
}

impl From<url::ParseError> for AppError {
    fn from(value: url::ParseError) -> Self {
        Self::InvalidUrl(value.to_string())
    }
}
