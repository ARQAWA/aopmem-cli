# RC5 Stage 017 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next stage: `STAGE_018`

Verified through: `STAGE_015`

Next cumulative audit: `STAGE_020`

Native Windows runtime: `PENDING_DOGFOOD`

P1: `0`

P2: `0`

## Result

Implemented one ownership-taking Atomic Publish V2 boundary in
`src/platform_publish.rs`.

Migrated audit snapshot, Git object/ref, backup, debug capsule, and managed
adapter/assets publication. Removed the obsolete
`SetFileInformationByHandle(FileRenameInfo)` regular-file publish path and
all caller use of the old `AnchoredDir` publish methods.

Windows uses `ReplaceFileW(..., flags=0)` for validated existing destinations
and `MoveFileExW(MOVEFILE_WRITE_THROUGH)` for absent/no-replace destinations.
It supports verbatim long paths, bounded safe race handling, and typed partial
state. Unix uses anchored `renameat` and `linkat + unlinkat`.

The helper is `O(1)`, path-private, reparse-safe, source-owning, flushes before
close, reopens the final file, and validates identity and size.

## Proof

```text
focused suites
PASS 93/93

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 716/716

rtk cargo test --tests --locked
PASS 716/716

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

No Windows target exists locally. Cross-build and native execution remain
pending. Continue with Stage 018 `aopmem platform check --json`.
