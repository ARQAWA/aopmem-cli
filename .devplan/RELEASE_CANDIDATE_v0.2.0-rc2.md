# AOPMem v0.2.0-rc2 Release Candidate

Status: approved for release.

## Scope

This candidate contains performance and maintainability improvements only.
Product behavior, public CLI and JSON contracts, schemas, cursor formats, and
security boundaries remain unchanged.

## Changes

- response-local indexes remove repeated recall scans;
- cached SQLite statements and index-friendly predicates reduce query work;
- bounded partial selection avoids sorting complete report result sets;
- storage, reflection, HTTP, and UI paths perform fewer clones and allocations;
- parity, query-plan, and negative regression tests cover optimized paths.

## Compatibility

- Fresh macOS Apple Silicon and Windows x64 installs are supported.
- The guarded installer upgrade path remains `v0.1.0-rc3` to `v0.2.0-rc2`.
- Automatic installer upgrade from `v0.2.0-rc1` is not supported.
- Native Windows execution remains a Windows-host validation boundary.

## Release proof

The release is accepted only after formatting, all-target tests, strict
Clippy, all-target build, JavaScript syntax, installer audit, artifact format,
version, checksum, and Git drift checks pass on the release worktree.
