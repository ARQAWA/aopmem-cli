# RC6 macOS isolated proof

Status: `PASS`

Host: Darwin arm64. This proof does not claim native Windows runtime success.

## Candidate and source assets

- Local RC6 candidate: `aopmem 0.2.0-rc6`.
- Candidate SHA-256: `012f9af58e642abda189ad46d54bc9f365cb72bb07ba49d1cc3b51b1134d1fa5`.
- Published RC4 macOS asset SHA-256:
  `4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e`.
- Published RC5 macOS asset SHA-256:
  `594bb9606bd7f971a0fb97b16916fe2a5da84096e8340a5885c36d7037dd1b5e`.

The proof downloaded each old asset and its `SHA256SUMS` through GitHub CLI,
then verified the exact asset line, hash, and `--version` before fixture use.

## Isolated installer and update proof

Command: `scripts/prove_rc6_macos.sh`.

The script proved:

- fresh RC6 install;
- published RC4 fixture (`schema 003`) upgraded to RC6;
- published RC5 fixture (`schema 004`) upgraded to RC6;
- two-workspace mixed `003`/`004` state reaches target schema `004`;
- staged platform check before audit repair, prepare, plan, one apply, publish,
  adapter sync, post-publish audit repair, health, and capsule export;
- exactly one apply and zero update onboarding questions;
- exact source SQLite bytes retained by the durable RC6 full-home backup;
- old RC5 executable retained in that full-home backup;
- old logical workflow canaries readable after update;
- doctor/verify, audit repair, task start/apply/complete, read-only dedupe
  plan, and debug export pass;
- no repository-local `.aopmem` directory is created.

The selected update workspace is healthy. The unselected schema-003 workspace
has the expected missing adapter block while its database, schema, audit, and
tool-directory checks remain ready.

## Separate UI loopback proof

Started the upgraded RC6 binary with `ui --no-open --port 0` in the isolated
schema-004 workspace. The printed `127.0.0.1` URL served the local HTML shell,
including its explicit read-only badge. The foreground test process was then
interrupted and confirmed absent.

## Boundary

Native Windows 11 / PowerShell 5.1 acceptance remains `PENDING_DOGFOOD`.
Run `.devplan/RC6_WINDOWS_ACCEPTANCE_PROMPT.md` after Stage 08 finalizes the
Windows asset SHA-256.
