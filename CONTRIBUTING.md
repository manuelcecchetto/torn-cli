# Contributing

Thanks for helping improve `torn-cli`.

## Development setup

```bash
git clone https://github.com/manuelcecchetto/torn-cli.git
cd torn-cli
cargo test --all-targets
```

Optional local keys belong in an ignored `.env` file copied from `.env.example`. Do not commit real keys.

## Checks before a pull request

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo package --allow-dirty --no-verify --list
```

The package list must not contain `.env`, `.env.local`, local cache databases, logs, crash dumps, or generated junk.

## Privacy checklist

For any change touching config, auth, requests, logging, output, cache, saved requests, tests, or TUI screens, verify:

- [ ] Torn keys are sent with `Authorization: ApiKey`, not query strings.
- [ ] FFScouter `key=` values are redacted in displayed/logged URLs.
- [ ] `config show`, verbose logs, errors, and snapshots do not reveal keys.
- [ ] Cache keys and metadata exclude raw secrets.
- [ ] Saved requests store templates only, not resolved auth secrets.
- [ ] Tests use fake placeholder keys and no private API responses.
- [ ] Optional online tests are skipped by default and documented.

## Documentation expectations

Update `README.md` or `docs/` when changing:

- command names, flags, or output contracts
- configuration variables or precedence
- endpoint index behavior
- cache behavior or stored file locations
- privacy/security guarantees
- installation or release process

## API behavior

Prefer generic request support over one-off endpoint logic. Shortcuts should be thin wrappers over the same request model used by generic commands and the TUI.

Mock external services in automated tests. Live Torn/FFScouter calls should be local, opt-in smoke tests only.

## Issues and pull requests

When filing bugs:

- Include command lines with keys removed.
- Redact response bodies if they contain private player/faction data.
- Prefer minimal mock payloads over real API responses.
- Mention OS, terminal, and `torn --version` when relevant.

## License

By contributing, you agree that your contribution is licensed under the MIT license in `LICENSE`.
