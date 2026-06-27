# AOPMem v0.1.0-rc1 Release Candidate

## Status

RC ready.

## Evidence

- Global audit: `.devplan/GLOBAL_AUDIT_REPORT.md` verdict is `PASS`.
  The only release risk, `GA-001`, is documented as resolved by final
  decision update.
- GA001 re-audit: `.devplan/GA001_REAUDIT_REPORT.md` verdict is `PASS`.
  No new P1/P2 findings were found.
- Ledger: `.devplan/EXECUTION_LEDGER.json` has 55 stages, all with status
  `VERIFIED`.
- Build/test:
  - `rtk cargo build`: PASS.
  - `rtk cargo test`: PASS, 164 passed.
  - `rtk cargo test --tests`: PASS, 164 passed.
  - `git diff --check`: PASS.
- Temp install proof:
  - copied `dist/aopmem-darwin-arm64/aopmem` to temp
    `AOPMEM_HOME/bin/aopmem`;
  - ran `aopmem --json init` in a temp repo with the 5 install answers;
  - ran `aopmem --json adapter seed --file AGENTS.md`;
  - ran `aopmem --json adapter status --file AGENTS.md`;
  - ran `aopmem --json doctor`;
  - ran `aopmem --json recall`;
  - confirmed one workspace under temp `AOPMEM_HOME/workspaces`;
  - confirmed `aopmem.sqlite`, `tools`, `artifacts`, `logs`, `runtimes`,
    and `audit-git`;
  - confirmed no `.aopmem` in the target repo;
  - confirmed `.understand.docs` is absent when Understand Anything is
    disabled;
  - confirmed the managed AOPMem block is `in_sync`.
- Binary proof:
  - `rtk bash scripts/build_macos_arm.sh`: PASS.
  - `file dist/aopmem-darwin-arm64/aopmem`: Mach-O 64-bit executable arm64.
  - `dist/aopmem-darwin-arm64/aopmem --version`: `aopmem 0.1.0`.
  - SHA-256:
    `798af720030081367969fb36a2913de98956d700fbdd6e87ae176d4e05caaefc`.

## Included Scope

- One Rust crate: `aopmem`.
- macOS ARM / Apple Silicon CLI binary.
- User-level runtime under `~/.aopmem` or `AOPMEM_HOME`.
- Per-repo workspace storage under `workspaces/<workspace-key>`.
- SQLite memory with typed nodes, links, events, registries, FTS5, and BM25.
- Core CLI flows: init, status, doctor, verify, recall, node/link/source/tag,
  MCP registry, tool registry/runner, artifacts cleanup, remember, teach, and
  reflect.
- Managed adapter block for agent instruction files.
- Optional `.understand.docs` only when Understand Anything is enabled.
- Local SQL audit snapshot under workspace `audit-git/memory.sql`.

## Explicitly Out of Scope

- Linux, Windows, and non-Apple-Silicon builds.
- GitHub Actions / CI.
- Mem0, Hindsight, Qdrant, vector search, semantic search, and embeddings.
- Custom MCP server implementation.
- Background enrichment or daemon behavior.
- Old MVP import/migration.
- Markdown memory import/export implementation.
- Global host memory as a product feature.
- Any new feature beyond the v0.1 final decisions.

## Install Path

The user install path is `install/v0.1/install_prompt.md`.

Expected release flow:

1. Publish the repo/release artifact for `v0.1.0-rc1`.
2. Provide the user with `install/v0.1/install_prompt.md`.
3. The installing agent checks macOS ARM silently.
4. The agent installs the macOS ARM binary to `~/.aopmem/bin/aopmem`.
5. The agent initializes the current repo under
   `~/.aopmem/workspaces/<workspace-key>`.
6. The agent asks only the 5 install questions from the prompt.
7. Optional Understand Anything and Codebase Memory MCP setup stays
   best-effort.
8. The agent inserts or updates only the managed AOPMem block.
9. The agent runs `aopmem doctor` and the first recall bundle.

## Known Non-Blocking Notes

- `GA-002` P3: root recovery docs path is not the canonical pack path. This
  is documentation/bookkeeping only and is not release-blocking.
- `GA-003` P3: the Stage 055 handoff text is stale, while ledger/proof show
  `STAGE_055` verified. This is not release-blocking.
- `GA-004` INFO: the repo baseline is mostly untracked. This is expected for
  the current proof baseline and is not a runtime issue.
- Optional MCP status `configured_unverified` is valid and non-blocking when
  the CLI has no deterministic detector.

## Release Recommendation

Yes, this repo can be pushed to GitHub and tested as `v0.1.0-rc1`.

Release checklist:

- Tag or label the candidate as `v0.1.0-rc1`.
- Include `dist/aopmem-darwin-arm64/aopmem` as the macOS ARM binary artifact.
- Include `install/v0.1/install_prompt.md` as the user install entry point.
- Run one fresh user-style install test from the published artifact before
  promoting to final `v0.1.0`.
