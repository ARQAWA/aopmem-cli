# RC7 global audit report

Status: `VERIFIED`

The final clean audit was performed directly after the user prohibited further
subagent use. No native Windows runtime claim was inferred from macOS evidence.

## Findings

| Severity | Open |
| --- | ---: |
| P1 | 0 |
| P2 | 0 |

One P2 was found during the first pass: streamed-body failures could report a
PowerShell `MethodInvocationException` wrapper instead of the exact inner
exception type. `Get-OriginalTransportException` now unwraps only
`MethodInvocationException` and `TargetInvocationException` for request and
stream failures. The transport harness and full installer audit were extended
and rerun. No P1/P2 remains.

## Twenty sweeps

| # | Sweep | Result |
| ---: | --- | --- |
| 1 | field failure transcription | PASS |
| 2 | proxy precedence | PASS |
| 3 | proxy credential isolation | PASS |
| 4 | PowerShell 5.1 API compatibility | PASS; static, native pending |
| 5 | redirect status handling | PASS |
| 6 | relative redirect resolution | PASS |
| 7 | HTTPS and no-userinfo policy | PASS |
| 8 | redirect loops and limits | PASS |
| 9 | missing `Response` regression | PASS |
| 10 | original exception preservation | PASS |
| 11 | streaming, cleanup, no-overwrite | PASS |
| 12 | known-source matrix | PASS; tagged RC1–RC6 plus v0.1 |
| 13 | apply-once installer order | PASS |
| 14 | backup and data preservation | PASS |
| 15 | RC6 feature regression | PASS |
| 16 | docs and privacy | PASS |
| 17 | version, assets, checksums | PASS |
| 18 | standalone acceptance prompt | PASS |
| 19 | forbidden-scope drift | PASS |
| 20 | stop and release conditions | PASS |

## Final clean commands

```text
cargo fmt --all -- --check
  PASS
cargo clippy --all-targets --locked -- -D warnings
  PASS; no issues
cargo build --locked
  PASS
cargo test --locked
  PASS; 771
cargo test --tests --locked
  PASS; 771
scripts/dev_verify.sh
  PASS; 769 unit + 2 integration and CLI proof
scripts/audit_v020_installers.sh
  PASS; 30 transport cases, 14 groups
git diff --check
  PASS
shasum -a 256 -c dist/SHA256SUMS
  PASS
jq ledger validation
  PASS; exactly 9 stages and valid statuses
forbidden drift scan
  PASS
```

## Release facts

- RC6 native Windows binary `platform check --json`: field PASS;
- RC6 installer transport: field FAIL behind required proxy;
- root defect: absent `Exception.Response` property masked the original error;
- RC7: explicit/env/system/direct proxy resolution and bounded manual
  `HttpClient` redirects;
- known-source warning: exact RC1–RC6 hashes recognized per platform;
- macOS fresh plus RC4/RC5/RC6 update proof: PASS;
- Windows PE x86-64 asset: two unchanged-source builds, identical SHA-256;
- schema: `004_task_protocol_and_tool_aliases`; no migration `005` added;
- native RC7 Windows PowerShell 5.1 proxy acceptance: `PENDING_DOGFOOD`.
