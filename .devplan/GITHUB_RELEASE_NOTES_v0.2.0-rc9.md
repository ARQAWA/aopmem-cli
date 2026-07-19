# AOPMem v0.2.0-rc9

RC9 removes the штатный updater and makes installation clean-only.

## Changed

- Removed the legacy upgrade CLI surface and updater runtime.
- Replaced installer behavior with clean install only.
- Added fail-closed populated-home detection with
  `CLEAN_INSTALL_REQUIRES_EMPTY_HOME`.
- Added an external Windows RC4 to RC9 transplant harness.
- Kept current SQLite schema at `004_task_protocol_and_tool_aliases`.

## Windows transition

Use quarantine, clean install, logical transplant, verify, and retain the
quarantine. The transplant harness is external and not installed into
`AOPMEM_HOME`.

Native Windows transplant execution remains pending for real user data.
