# RC7 Current Stage

Status: `STAGE_09_AWAITING_EXTERNAL_GATE`

Objective: wait for standalone `+++`, then execute the exact drafted
push/tag/prerelease operations and verify remote assets.

Baseline:

- branch: `main`
- local and `origin/main`:
  `b47ff96681c77aa18a4baa07e030fa2dc78eeb88`
- immutable baseline release: `v0.2.0-rc6`
- target release: `v0.2.0-rc7`
- target schema: `004_task_protocol_and_tool_aliases`

Stage 01: `VERIFIED`.
Stage 02: `VERIFIED`.
Stage 03: `VERIFIED`.
Stage 04: `VERIFIED`.
Stage 05: `VERIFIED`; full regression and isolated RC4/RC5/RC6 to RC7 macOS
proofs passed.
Stage 06: `VERIFIED`.
Stage 07: `VERIFIED`; final clean audit P1=0, P2=0.
Stage 08: `VERIFIED`; the commit containing this record is the one audited RC7
release commit.
Stage 09: `PENDING`.

Next: emit the exact External Action Draft and stop until standalone `+++`.
