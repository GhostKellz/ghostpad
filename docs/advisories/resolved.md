# Resolved Advisories

Security advisories that previously appeared in `cargo audit` / `cargo deny`
output and have since been cleared. The dependency-modernization pass also
removed potential future exposure by dropping an unused dependency and bumping
several crates to current major versions.

**No active advisories have been recorded against GhostPad.** `cargo audit` was
clean both before and after the dependency updates below; these entries document
the preventive maintenance rather than a fix for a reported vulnerability.

| Change | Crate | From → To | Rationale | Date |
|--------|-------|-----------|-----------|------|
| Removed unused dependency | `notify-debouncer-mini` | 0.4 → (removed) | Declared but never used; dropping it shrinks the dependency surface | 2026-06-17 |
| Major bump | `notify` | 6 → 8 | Stay on the maintained release line for the file watcher | 2026-06-17 |
| Major bump | `directories` | 5 → 6 | Stay on the maintained release line for config-path resolution | 2026-06-17 |
| Major bump | `cxx-qt` / `cxx-qt-lib` / `cxx-qt-build` | 0.7 → 0.8 | Current Qt bindings; new QML-module registration API | 2026-06-17 |

After these changes `cargo audit` reports zero vulnerabilities and zero
warnings, and `deny.toml` carries an empty `[advisories] ignore` list.
