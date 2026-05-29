# Security Policy

## Reporting security vulnerabilities
If you discover a security issue in `kite-tui`, report it privately first. **Do not** open a public issue for security vulnerabilities.

- Preferred channel: [GitHub Security Advisory](https://github.com/KernelFreeze/kite-tui/security/advisories/new)
- Backup channel: `celeste@etheryal.net`

Include the following in the initial report:

- Affected versions (`kite-tui` version from `kite --version` or crate version)
- Your OS and architecture
- Installation method (`cargo install`, release artifact, installer script, package manager)
- Exact command line and environment used
  - `KITE_BASE_URL` / `--base-url`
  - `KITE_TIMEOUT_SECONDS`
  - `RUST_LOG`
  - any startup flags
- Reproduction steps in as few steps as possible
- Minimal reproducible configuration (`settings.toml`) with secrets removed
- Relevant logs or debug output
- Network details
  - Endpoint/domain reached
  - Whether TLS was used
  - Any proxy/VPN/firewall context if relevant

## Scope

Report issues affecting:

- TLS/certificate failures or request tampering
- Insecure handling of remote content (`news.kagi.com` payload parsing)
- Local file handling for:
  - `settings.toml`
  - `read_articles.toml`
- Privilege escalation, local code execution, denial-of-service, or crash/recovery bypasses
- Installer/update or distribution integrity issues

Not in scope by default:

- Content quality or UI/UX issues
- Malicious output from the upstream `Kagi News` feed
- Non-reproducible crashes requiring third-party availability failures only

## Data and privacy expectations
- This app stores local preferences and read-state in TOML files under standard platform config/data directories and does not intentionally persist credentials.
- If your report includes user data, sanitize local paths/UUIDs and remove unrelated personal data before sharing.
- Do not include private credentials or any auth token. If accidentally included, rotate them before sharing.

## What we do next

Please allow up to **72 hours** for initial acknowledgment. We will:

1. Acknowledge receipt
2. Validate a minimal reproducer against a patched branch
3. Provide severity and impact
4. Coordinate a release if the fix is not backward-compatible

We may request additional context repeatedly; helpful follow-up is faster than perfect first reports.

## Non-security issues

For non-security bugs and feature requests, use the normal issue tracker.
