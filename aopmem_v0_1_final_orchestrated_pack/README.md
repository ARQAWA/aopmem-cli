# AOPMem v0.1 — Final Dev Pack

Этот пакет — финальное кумулятивное ТЗ и набор execution prompts для разработки **AOPMem v0.1** как отдельного Rust CLI-продукта.

Важное разделение:

1. **Dev repo** — отдельный репозиторий, где разрабатывается AOPMem CLI.
2. **Global install** — после публикации из GitHub AOPMem ставится в `~/.aopmem`.
3. **Workspace init** — AOPMem подключается к конкретному рабочему repo через managed block в instruction file и optional `.understand.docs`.

Код пишет **gpt-5.4** subagent с `reasoning_effort=medium`. Milestone
cumulative audit делает **gpt-5.4** audit subagent с
`reasoning_effort=high` каждые 5 stages. Пользователь запускает одного
orchestrator-а. Orchestrator запускает implementation subagents по stage
prompts, milestone audit subagents на `005`, `010`, `015`, `020`, `025`,
`030`, ... и patch subagents при необходимости.

## Главные решения

- macOS ARM only v0.1.
- Rust, один crate `aopmem`.
- Runtime storage: `~/.aopmem`.
- AOPMem memory: SQLite only.
- Search: SQLite FTS5/BM25 only.
- No semantic/vector search.
- No Mem0/Hindsight.
- No custom MCP server.
- No migration from old MVP.
- No markdown export/import for AOPMem memory.
- `.understand.docs` optional, Markdown canonical, local-only by default.
- Codebase Memory MCP optional.
- Corporate MCP registry exists but starts empty.
- Memory Keeper subagent required by agent contract.
- Generated tools are called via `aopmem tool run`, not directly.
- Reflection is user-triggered: low-risk auto-apply, high-risk draft.
- Hunch enabled: 1–3 source-backed memory hunches per recall bundle.
- Artifacts retention: 7 days or 1 GB per workspace, whichever comes first.
- DERC required: deterministic execution, recovery, coverage, proof, handoff.

## Files

- `reference/FINAL_DECISION_LOG.md` — финальные решения.
- `reference/NON_NEGOTIABLE_SCOPE.md` — что строго out of scope.
- `reference/PRODUCT_SPEC.md` — что строим и как это должно работать.
- `reference/DEV_REPO_SPEC.md` — структура Rust repo.
- `reference/RUST_IMPLEMENTATION_SPEC.md` — язык, модули, зависимости, проверки.
- `reference/STORAGE_AND_SQLITE_SPEC.md` — storage, schema, FTS, audit.
- `reference/CLI_CONTRACT.md` — MCP-like CLI contract.
- `reference/INSTALL_AND_WORKSPACE_INIT.md` — global install и workspace init.
- `reference/MEMORY_KEEPER_AND_REFLECTION.md` — Memory Keeper, reflection, hunch.
- `reference/TOOLS_AND_MCP_REGISTRY.md` — generated tools and MCP profiles.
- `reference/DERC_PROTOCOL.md` — deterministic execution/recovery contract.
- `reference/ORCHESTRATOR_EXECUTION_MODEL.md` — orchestrator/subagent execution flow.
- `reference/REQUIREMENTS_MATRIX.md` — requirements IDs and coverage.
- `reference/STAGE_GRAPH.md` — порядок разработки.
- `stage_prompts/` — 55 prompts для `gpt-5.4` medium subagents.
- `audit_prompts/` — prompts для `gpt-5.4` high audit subagents.
- `RUN_FIRST.md` — первый prompt для запуска разработки.

## Как использовать

1. Создать пустой dev repo AOPMem.
2. Распаковать этот пакет в repo или дать агенту доступ к файлам.
3. Запустить `RUN_FIRST.md` в orchestrator session.
4. Orchestrator сам запускает implementation subagent на STAGE_001.
5. Orchestrator сам запускает `gpt-5.4` high cumulative audit subagent на
   каждом milestone stage.
6. Если milestone audit нашел проблемы — orchestrator запускает patch
   subagent только по findings.
7. Orchestrator продолжает по `reference/STAGE_GRAPH.md` non-stop до
   следующего milestone audit и затем до финального proof.
