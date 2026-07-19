# Windows long-path support

RC8 must work on Windows when:

```text
LongPathsEnabled=0
```

The recovery boundary uses Rust filesystem operations and Windows verbatim
paths internally. User-facing manifests keep logical relative paths.

Required properties:

- no administrator rights;
- no registry change;
- no path shortening;
- no deletion of `.venv`, tools, runtimes, or secrets;
- no PowerShell `Copy-Item` fallback for recovery backup;
- no reparse escape;
- no destination inside source;
- no silent partial copy.

Field evidence proved Unicode, Cyrillic, and spaces work. It also proved
near-260 paths fail with ordinary Windows path handling. RC8 treats that as a
product boundary and uses long-path-safe traversal/copy/hash for recovery.
