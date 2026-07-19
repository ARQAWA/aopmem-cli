# RC8 Long Path Report

Field evidence:

- `LongPathsEnabled=0`.
- Unicode, Cyrillic, and spaces passed.
- Near-260 ordinary path failed with `WinError 206`.
- Deep runtime `.venv` paths exist in live home.

Implementation:

- Windows recovery traversal converts paths to verbatim paths at OS boundary.
- Logical manifest paths stay relative and do not expose `\\?\`.
- Recovery includes `.venv`, tools, and runtimes instead of deleting them.

Remaining proof:

- Native Windows acceptance must run the exact RC8 assets.
