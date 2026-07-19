# RC6 Current Stage

Status: `STAGE_10_LOCAL_COMMIT_IN_PROGRESS`

Objective: verify the exact local release tree, create one local RC6 commit,
then stop at the required external operation gate.

Baseline:

- branch: `main`
- local and `origin/main`: `d2eb26bc36b20349061bf89d23a152c8c0b161bf`
- baseline release: `v0.2.0-rc5`
- target release: `v0.2.0-rc6`

Stage 01: `VERIFIED`.
Stage 02: `VERIFIED`.
Stage 03: `VERIFIED`.
Stage 04: `VERIFIED`.
Stage 05: `VERIFIED`.

Stage 06: `VERIFIED`.
Stage 07: `VERIFIED`.
Stage 08: `VERIFIED`.
Stage 09: `VERIFIED`; P1 `0`, P2 `0`.

Next: create the local release commit. Push, tag, and GitHub prerelease remain
blocked until a standalone `+++` approval.
