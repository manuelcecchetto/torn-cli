# Security Policy

## Reporting vulnerabilities

Please do not open a public issue for vulnerabilities that could expose API keys, private Torn/FFScouter data, local files, or command execution.

Report privately to the repository owner through GitHub Security Advisories for `manuelcecchetto/torn-cli`, or use another private contact method if one is listed on the GitHub profile.

Include:

- affected version or commit
- operating system and shell if relevant
- minimal reproduction steps
- expected and actual behavior
- impact assessment
- redacted logs or screenshots only

Do not include real Torn or FFScouter API keys. If a proof of concept needs a key, use a fake placeholder and describe the required shape.

## Supported versions

The project is pre-1.0. Security fixes target the main branch until release branches exist.

## Security expectations

`torn-cli` should:

- keep API keys local
- avoid telemetry by default
- use Torn header auth rather than URL query auth
- redact FFScouter `key=` query values in displayed/logged URLs
- avoid storing raw secrets in saved requests, cache keys, logs, errors, snapshots, or crash output
- use fake keys in tests and fixtures

## Secret leaks

If you accidentally commit or disclose a real API key:

1. Revoke or rotate the key in the upstream service immediately.
2. Remove the secret from the repository history if it was committed.
3. Open a private security report describing where the leak happened.

Repository maintainers should treat any committed real key as compromised, even if it is later deleted.
