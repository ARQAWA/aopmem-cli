# Windows clean transplant

RC9 removes the in-place updater. The only supported Windows transition from an
old home is:

```text
stop AOPMem
quarantine whole old home
clean RC9 install
logical transplant
verify
retain quarantine
```

The clean installer never renames, deletes, or edits an existing populated
`%USERPROFILE%\.aopmem`. It fails before product mutation with:

```text
CLEAN_INSTALL_REQUIRES_EMPTY_HOME
existing AOPMem installation detected; quarantine it before clean installation
```

The external harness is:

```text
scripts/windows_rc4_to_rc9_transplant.ps1
scripts/windows_rc4_to_rc9_transplant.py
```

It is not part of the `aopmem` binary and is not installed into
`AOPMEM_HOME`.

Quarantine:

- preferred root: `C:\a4\<short-id>\home`;
- fallback root: `%USERPROFILE%\a4\<short-id>\home`;
- same volume as the live home where possible;
- never deleted automatically;
- source for rollback and forensic inspection.

Logical transplant:

- creates fresh RC9 workspace stores;
- opens old SQLite stores read-only from quarantine;
- copies canonical rows with explicit table maps;
- preserves IDs, references, timestamps, ordering, aliases, tags, sources,
  registries, MCP profiles, tool contracts, and workspace identities;
- rebuilds FTS from canonical rows;
- runs `foreign_key_check` and `quick_check`.

Filesystem transplant:

- copies user tools, runtimes, artifacts, and secret-container files;
- preserves relative paths;
- rejects reparse traversal and path escape;
- keeps RC9 built-in product files when conflicts exist;
- reports conflicts without printing secret values.

Excluded state stays in quarantine:

- old binary;
- `schema_migrations` rows;
- FTS shadow tables;
- WAL and SHM;
- mutation locks;
- old logs;
- old observability state;
- audit snapshots and audit-git;
- product caches and generated temporary files.

Rollback:

- any post-quarantine error moves incomplete RC9 home to `C:\a9f\<short-id>\home`
  or `%USERPROFILE%\a9f\<short-id>\home`;
- quarantined old home is renamed back to `%USERPROFILE%\.aopmem`;
- failed RC9 evidence and reports are retained;
- user data is never deleted.

Native Windows execution remains the required final proof for a real user home.
