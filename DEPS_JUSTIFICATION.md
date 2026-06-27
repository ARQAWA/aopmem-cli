# Dependencies Justification

Stage 003 adds only near-term CLI and storage dependencies.

Every future dependency must be recorded here with a short reason before use.

## Runtime dependencies

- crate: `clap`
  - Reason: stable command-line parsing for the required `aopmem` command groups.
- crate: `directories`
  - Reason: reliable user-level path discovery for `~/.aopmem` support.
- crate: `rusqlite`
  - Reason: SQLite access for the required per-workspace canonical memory DB.
  - Feature: `bundled` keeps SQLite/FTS5 availability deterministic on macOS ARM.
- crate: `serde`
  - Reason: derive support for stable JSON envelope and typed storage models.
- crate: `serde_json`
  - Reason: JSON machine output for CLI proof and error envelopes.
- crate: `thiserror`
  - Reason: typed internal errors without adding a broad application framework.
