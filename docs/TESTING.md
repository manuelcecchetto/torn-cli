# Testing

The default test suite must be safe to run without real Torn or FFScouter keys.

## Offline checks

Run these before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Packaging sanity check:

```bash
cargo package --allow-dirty --no-verify --list
```

The package list may include `.env.example`; it must not include `.env`, `.env.local`, cache databases, logs, or other local runtime files.

## Required privacy-focused tests

As implementation lands, keep tests for:

- redaction of long and short secrets
- config precedence and private `config set` writes without exposing values
- Torn auth attachment through `Authorization: ApiKey`
- FFScouter auth attachment through a query parameter
- displayed/logged URL sanitization for `key=`
- FFScouter response-body redaction when endpoints echo configured keys
- FFScouter shortcut parameter mapping (`targets`/`target`, activity windows, mutation `--yes` guards)
- `--watch` parsing, GET-only guard, cache-bypass policy, and colored `[time]` prefixes
- schema-aware `--pretty` summaries for user profile/status, member lists, FFScouter stats, and fallback wrappers
- query merging where explicit `--param` values override path query values
- cache key generation that excludes auth secrets
- saved request persistence without resolved auth secrets
- permission preflight for predefined, custom, and custom-log keys
- built-in log preset category coverage and user preset config round-trips
- error rendering without keys or private config values

Use fake keys such as:

```text
fake_torn_key_for_tests
fake_ffscouter_key_for_tests
```

Never use real keys in fixtures, snapshots, golden files, or CI secrets for the default suite.

## Safe optional online tests

Online tests are useful for local smoke testing, but they must be opt-in and skipped in CI unless explicitly configured.

Recommended workflow:

```bash
set +x
cp .env.example .env
chmod 600 .env
$EDITOR .env
```

Then run only explicit smoke commands:

```bash
torn config check --online
torn config permissions
torn logs presets list
torn logs presets run api-keys --limit 5 --json
torn api get /user/basic --pretty
torn api get /user --param selections=basic,bars --json
torn ff check-key --pretty
# Non-mutating FFScouter feature checks; premium endpoints may return code 19 on non-premium keys.
torn ff stats --target 1844049 --pretty
torn ff targets --preset respect --limit 5 --json
torn ff losses quote --quantity 10 --price-per-loss 300000 --pretty
# Requires a full-access Torn key; body may contain private account activity.
torn logs analyze --since 24h --to now --group-by category --table
torn logs catalog --no-expand --pretty
```

Safety rules:

- Do not run online tests with shell xtrace (`set -x`).
- Do not paste real keys into command lines, issue trackers, pull requests, or chat tools.
- Prefer `torn config set`, `torn config tui`, `.env`, process environment variables, or an OS secret manager.
- Use minimum-access Torn keys for the endpoints being tested; use full-access Torn keys only for `/user/log` tests.
- Review terminal output before sharing; keys, FFScouter `key=` values, and private user-log payloads must be redacted.
- Stop retrying if the API reports invalid, disabled, paused, or access-too-low keys.

## Mocking external APIs

Unit and integration tests should use local mock servers. Mock fixtures should include representative success and error payloads, including Torn JSON error bodies returned with HTTP 200. Do not rely on live services for deterministic tests.

## Rate-limit-sensitive behavior

Tests for retry/backoff and pagination should use mocks. Real Torn usage should stay under the documented 100 requests/minute per-user limit and should respect service-cache behavior.
