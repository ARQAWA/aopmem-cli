# Backup inventory policy

RC8 uses one Rust inventory engine for:

- recovery copy plan;
- source manifest;
- backup manifest;
- post-copy verification;
- adoption validation;
- tests.

Included persistent categories:

- `bin/**`
- `skills/**`
- `templates/**`
- `workspaces/**`
- workspace tools and launchers;
- workspace runtimes;
- `.venv`;
- artifacts;
- audit state;
- observability;
- logs;
- `.pending-snapshot`;
- local secret/config containers.

Excluded product ephemeral paths:

- `workspaces/*/.mutation.lock`
- `workspaces/*/aopmem.sqlite-wal`
- `workspaces/*/aopmem.sqlite-shm`
- `bin/.aopmem-v*.tmp`

SQLite databases are copied with SQLite Online Backup and then reopened
read-only for integrity/schema checks.

The installer Safety Backup is whole-home emergency evidence. It may preserve
more historical material than the Upgrade Recovery Backup. It is not normal
recovery journal state.
