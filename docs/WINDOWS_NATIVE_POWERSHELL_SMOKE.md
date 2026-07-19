# Windows native PowerShell smoke

Use this smoke only after RC8 assets exist on GitHub.

Required result:

```text
aopmem 0.2.0-rc8
doctor healthy=true
verify clean=true
task_start ok=true
observability ok=true
```

Acceptance must prove:

- native Windows 11 x64;
- Windows PowerShell 5.1;
- non-admin user;
- proxy path or direct path;
- `LongPathsEnabled=0`;
- no repository-local `.aopmem`;
- no normal `upgrade backup --adopt`;
- Safety Backup retained;
- Upgrade Recovery Backup retained;
- recovery journal schema v1;
- apply attempts exactly one;
- `.venv`, tools, runtimes, secrets containers, audit evidence, and pending
  markers preserved;
- `.mutation.lock`, WAL, and SHM treated as ephemeral;
- debug capsule exported.
