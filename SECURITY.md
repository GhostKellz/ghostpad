# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in GhostPad, please report it
responsibly:

1. **Do not** open a public GitHub issue for security vulnerabilities.
2. Report privately via GitHub Security Advisories
   ("Report a vulnerability" on the repository's Security tab).
3. Include detailed steps to reproduce the issue.
4. Allow reasonable time for a fix before public disclosure.

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Security Practices

GhostPad is a local desktop text editor. Its attack surface is deliberately
small:

- **No network access.** GhostPad makes no network requests. It neither phones
  home nor fetches remote content; all data stays on the local machine.
- **Local file I/O only.** The editor reads and writes files the user explicitly
  opens or saves, plus its own config and autosave files under
  `~/.config/ghostpad/` (see [configuration](docs/getting-started/configuration.md)).
- **File-size guard.** Files larger than 25 MB are rejected to avoid unbounded
  memory use from a single document.
- **External-change detection.** Open files are watched for on-disk
  modification/deletion; the user is prompted before reloading, so external
  changes never silently overwrite the buffer.
- **Autosave isolation.** The autosave snapshot is written to a separate
  `config.autosave.json` and never overwrites the user's real files on disk.
- **Minimal `unsafe`.** Rust `unsafe` is limited to the cxx-qt FFI boundary and
  the small environment-variable setup in `main.rs`.

## Dependency Auditing

We use `cargo audit` and `cargo deny` to track known vulnerabilities and
unmaintained crates.

```bash
cargo install cargo-audit cargo-deny
cargo audit
cargo deny check
```

Run these locally before submitting changes. Dependabot is also enabled for the
repository.

### Current Audit Status

`cargo audit` and `cargo deny check advisories` both pass with no vulnerabilities
and no accepted (ignored) advisories. Advisory history is tracked under
[`docs/advisories/`](docs/advisories/):

- [`docs/advisories/accepted.md`](docs/advisories/accepted.md) — knowingly
  accepted advisories and their matching `deny.toml` ignore entries (currently
  none).
- [`docs/advisories/resolved.md`](docs/advisories/resolved.md) — advisories and
  maintenance changes that cleared past or potential exposure.

`deny.toml` is the authoritative ignore list and is kept in sync with
`docs/advisories/accepted.md`. Version history lives in
[`CHANGELOG.md`](CHANGELOG.md).
