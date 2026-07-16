# AOPMem v0.2.0-rc1 Release Candidate

Status: ready for macOS and Windows dogfood.

## Candidate outcome

The implementation is ready for macOS and Windows dogfood. The independent
final audit reports open P1=0, P2=0, and P3=0. This is not a public release.

## Preserved optimization work

- streaming SQL dump and canonical FTS rebuild;
- atomic snapshot publication, real LocalGitAudit commits, and pending marker;
- read-only DB opens, pending-only migrations, summary/direct metadata indexes;
- transactional teach/reflect, batched FTS, and atomic draft tool creation;
- tool path containment and concurrent stdout/stderr drains;
- lean lint projection, direct tool/rule recall, boxed large CLI variant;
- dry-run without execution and validation before DB access.

## Removed conflicting optimization behavior

- removed the rule that every draft tool requires `+++`;
- removed `draft_review`;
- removed `Draft tool execution requires +++.` from the managed block;
- restored approval by actual contract, side effect, and explicit risk.

## Added in v0.2.0-rc1

- explicit complete keyset pagination and controlled `--all` traversal;
- mandatory-safe query recall with continuation, selection reasons, and
  debug-only `--full`;
- per-tool timeout/output/artifact contracts and global ceilings;
- one reflection inventory projection with append-only event history;
- separate local observability, bundle correlation, feedback, and fact report;
- deterministic redacted 12-entry debug capsule;
- embedded loopback/token/read-only six-view desktop UI;
- read-only upgrade planning, guarded backups/migrations, safe resumable apply;
- native prebuilt-binary fresh/update installers for the two supported targets.

## Final proof summary

| Item | Result |
|---|---|
| Rust tests | 575/575, twice |
| `dev_verify` | PASS, including another 575-test run |
| Installer audit | 11/11 groups |
| Real macOS fresh | adapter in-sync; doctor healthy; verify clean |
| Real v0.1 update | exact logical/tool/artifact preservation; 001/002/003 |
| Observability | real status/report/export; 12-entry redacted capsule |
| UI | live 200/invalid-token 404/POST 405; 13 HTTP tests; 3 screenshots |
| Benchmark | 1,060 measured samples; raw JSON/CSV; no percentage claim |
| Independent findings | open P1=0, P2=0, P3=0 |

## Binaries

| Platform | Asset | SHA-256 |
|---|---|---|
| macOS Apple Silicon | `dist/aopmem-darwin-arm64` | `b32e918d2a44f0767444e09c84c1ed44fe9177709b2d56b2aa89c300081d4308` |
| Windows 11 x64 | `dist/aopmem-windows-x86_64.exe` | `a4e3302d6f26dd9d16387a075189fec51c469aef9b8d9c730f81001b21b2cf57` |

The macOS asset is Mach-O arm64, minimum macOS 11.0, and not stripped. The
Windows asset is PE32+ x86-64 and has no dynamic MSVC/UCRT import. Native
Windows execution remains the first Windows dogfood task; it was not possible
on the macOS build host.

## Migration status

The exact peeled-tag v0.1 fixture updates to schema migrations `001,002,003`.
Nodes, links, aliases, tags, sources, events, registries, tool contracts, MCP
profiles, generated tool bytes, artifacts, and adapter backup are preserved.
Update asks no onboarding questions. All backups remain present.

## UI proof

The final binary served embedded assets and bounded read APIs only on a random
`127.0.0.1` port with a random token. Invalid token returned 404 and POST
returned 405. Overview, Graph, and Activity proofs are true 1440x900 PNGs.

## Observability proof

The migrated fixture has a separate schema-v1 observability database with
`update.started` and `update.completed`. Status/report/export work without
changing operational memory. The real capsule contains exactly 12 redacted
entries and no raw task, node body, tool output, token, or environment dump.

## Remaining validation boundary

- Independent final audit has no open P1, P2, or P3.
- Native Windows execution is intentionally deferred to Windows dogfood.
- Accepted P3 boundaries remain documented in the global audit and decisions
  D-017, D-021, D-022, D-028, and D-030.

## Stop condition

Do not push, tag, create a GitHub Release, install into a real user workspace,
or delete backups. The independent audit and ledger are complete; stop here.
