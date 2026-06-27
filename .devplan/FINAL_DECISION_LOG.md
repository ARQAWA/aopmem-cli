# FINAL DECISION LOG — AOPMem v0.1

Этот файл является source of truth для всех решений. Любое противоречие между stage prompt и этим файлом решается в пользу этого файла.

## 1. Product boundary

AOPMem v0.1 — отдельный Rust CLI-продукт, разрабатываемый в собственном dev repo.

Он не разрабатывается внутри рабочего repo, куда потом будет установлен. После публикации в GitHub пользователь дает агенту ссылку на install prompt, и агент устанавливает AOPMem глобально на host, затем инициализирует конкретный workspace.

## 2. Runtime install model

AOPMem имеет три контекста:

1. **Dev repo** — исходники, тесты, proof, install prompt, бинарь.
2. **Global host installation** — `~/.aopmem`.
3. **Workspace init** — подключение AOPMem к конкретному repo.

## 3. OS support

v0.1 поддерживает только **macOS ARM / Apple Silicon**.

Linux, Windows и другие архитектуры out of scope.

## 4. Build/install model

В dev repo собирается готовый бинарь для macOS ARM. Он может быть сохранен в repo/release artifact.

Install prompt должен уметь установить этот бинарь в user-level AOPMem backend.

Source build и prebuilt release можно описывать как техническую возможность, но v0.1 ориентирован на macOS ARM binary.

## 5. Rust layout

Один Rust crate `aopmem`.

Внутренние модули:

- `cli`
- `storage`
- `schema`
- `recall`
- `install`
- `tools`
- `reflection`
- `verify`
- `audit`
- `artifacts`
- `adapter`

Cargo workspace с несколькими crates out of scope для v0.1.

## 6. Dependencies

Dependencies allowed, но каждый dependency должен иметь короткое justification в `DEPS_JUSTIFICATION.md`.

Gemini/Codex не должен тянуть crate без записи причины.

## 7. CI

GitHub Actions / CI out of scope.

Все проверки локальные:

- `cargo build`
- `cargo test`
- CLI proof scenarios
- negative checks
- drift checks
- reproducible proof files

Форматтеры/линтеры не являются обязательным gate в v0.1.

## 8. Development actors

- Код пишет **gpt-5.4** subagent с `reasoning_effort=medium`.
- **gpt-5.4** audit subagent с `reasoning_effort=high` запускается
  cumulative milestone audit каждые 5 stages.
- Milestones: `005`, `010`, `015`, `020`, `025`, `030`, `035`, ...
- Между milestone audits stage может завершиться как
  `DONE_LOCAL_CHECKS_PASSED`.
- Gemini Flash больше не является обязательным исполнителем в финальной модели, но prompts остаются достаточно детерминированными для слабого агента.
- По умолчанию активен только **один** subagent.
- Старый completed/stale subagent надо закрыть **до** нового spawn.
- Если spawn или wait висит 10 минут, orchestrator запускает watchdog:
  закрывает stale subagent, перечитывает DERC state из файлов и продолжает
  тот же thin slice локально или одним fresh replacement subagent.

## 9. DERC

Development обязано следовать DERC: Deterministic Execution & Recovery Contract.

Каждый stage:

- читает decision log;
- выполняет только свой scope;
- меняет только allowed files;
- не добавляет features сверх ТЗ;
- выполняет локальный proof;
- обновляет ledger;
- пишет handoff;
- останавливается.

AUTO_PATCH_WINDOW:

- если stage уперся в обычный соседний слой, orchestrator не стопается и не
  ждет пользователя;
- orchestrator открывает минимальный `AUTO_PATCH_WINDOW`;
- patch subagent правит только нужный adjacent-layer wiring из dependency
  matrix;
- patch записывается в proof и ledger;
- patch не начинает следующий stage;
- stage flow продолжается дальше;
- milestone audit потом проверяет весь cumulative state.

Dependency scope matrix:

```text
CLI stage:
  primary: src/cli/**
  auto_patch_allowed: src/storage/**, src/recall/**

Storage stage:
  primary: src/storage/**
  auto_patch_allowed: src/schema/**, src/types/**

Recall stage:
  primary: src/recall/**
  auto_patch_allowed: src/storage/**, src/cli/**

Tool stage:
  primary: src/tools/**
  auto_patch_allowed: src/cli/**, src/storage/**

Install stage:
  primary: src/install/**
  auto_patch_allowed: src/workspace/**, src/cli/**

Reflection stage:
  primary: src/reflection/**
  auto_patch_allowed: src/storage/**, src/cli/**

Verification stage:
  primary: tests/**, scripts/**
  auto_patch_allowed: src/cli/** only for testability wiring
```

AUTO_PATCH_WINDOW обязательно записывается в `.devplan/PROOF_LOG.md` и
handoff. Audit обязан проверить:

```text
patch touched only dependency matrix files
patch did not start next stage
patch did not add features
patch only fixed blocker
all required checks pass
```

`BLOCKED` разрешен только для настоящих блокеров:

```text
нужно нарушить out-of-scope
нужно добавить запрещенную feature
нужно изменить FINAL_DECISION_LOG
нужно поменять архитектурное решение
нужно тронуть high-risk contract
нужен новый dependency без justification
нужен файл вне dependency matrix
требуется действие во внешней системе
required checks не проходят и нет deterministic patch
```

## 10. Runtime storage

Все runtime-данные хранятся в user-level backend:

```text
~/.aopmem/
  bin/
  skills/
  templates/
  workspaces/
    <workspace-key>/
      aopmem.sqlite
      tools/
      artifacts/
      audit-git/
      runtimes/
      logs/
```

В рабочий repo не кладется `.aopmem`.

## 11. Workspace key

Workspace key формируется детерминированно:

```text
<sanitized-repo-folder-name>-<8-char-path-hash>
```

Пользователь не вводит project ID.

## 12. Global memory

Global host memory out of scope.

Разрешен только технический registry/index внутри `~/.aopmem`, если он нужен для сопоставления repo path → workspace.

## 13. What remains in target repo

В рабочем repo остается только:

- managed AOPMem block в текущем agent instruction file;
- optional `.understand.docs`, если включен Understand Anything.

Instruction file зависит от shell:

- Codex/OpenAI: `AGENTS.md`
- Claude: `CLAUDE.md`
- Cursor: Cursor rules
- GitHub Copilot: Copilot instructions

Installer должен seed-ить только adapter текущей среды. Не генерировать все подряд.

## 14. Adapter block

Adapter block вставляется между markers:

```md
<!-- AOPMEM:BEGIN managed block -->
...
<!-- AOPMEM:END managed block -->
```

Если block есть — обновлять только block.
Если block поврежден — остановиться с ошибкой, не чинить молча.

## 15. AOPMem memory storage

AOPMem canonical memory — только SQLite.

Нет Markdown exports/views/imports для AOPMem memory.

Человек не редактирует память руками. Человек управляет памятью через агента.

## 16. Search

Только:

- structured retrieval;
- typed links traversal;
- SQLite FTS5/BM25.

Запрещено в v0.1:

- semantic search;
- vector search;
- embeddings;
- Qdrant;
- RAG engine.

## 17. LLM Wiki pattern

LLM Wiki в AOPMem реализуется через SQLite:

- cards/pages = `nodes`;
- wikilinks = `links`;
- index/maps = SQL queries/views;
- log = `events`;
- raw = `raw_note` nodes;
- recall = Memory Keeper traversal + bundle.

## 18. Zettelkasten-like atomicity

Atomic one-node-one-idea применяется только к:

- rules;
- corrections;
- decisions;
- failure modes;
- lessons.

Workflows могут быть длиннее.

## 19. Understand Anything

Optional.

Если включен:

- создается `.understand.docs`;
- local-only по умолчанию;
- installer может best-effort подключить/индексировать;
- product/code/domain/architecture knowledge идет туда.
- optional MCP profile status follows the final optional MCP status contract:
  `disabled`, `installed`, `missing`, or `configured_unverified`.
- `configured_unverified` is valid and non-blocking when the user enables
  Understand Anything but the CLI cannot reliably verify the agent-local or
  host-global capability.

Если Understand выключен, product knowledge fallback в AOPMem как `project_fact`.

## 20. Codebase Memory MCP

Optional.

Нужен для code navigation. В v0.1 AOPMem регистрирует profile и
best-effort подключение, но не обязан выполнять реальные MCP calls.

Optional MCP status contract:

- `disabled`: user did not enable this optional MCP/tool.
- `installed`: CLI can verify local executable/capability using a deterministic
  detector.
- `missing`: CLI can run a deterministic detector and it fails.
- `configured_unverified`: user enabled the MCP/tool, but CLI cannot reliably
  verify it because it is agent-local, host-global, shell-managed, or otherwise
  outside deterministic CLI detection.

Rules:

1. enabled + detector pass -> `installed`.
2. enabled + detector fail -> `missing`.
3. enabled + no reliable detector -> `configured_unverified`.
4. disabled -> `disabled`.
5. `configured_unverified` is valid and non-blocking for v0.1.
6. install must not fail because optional MCP is `missing` or
   `configured_unverified`.
7. Memory Keeper/agent may still use the tool if the agent shell exposes it.
8. AOPMem CLI must not fake `installed` without deterministic evidence.

## 21. Corporate MCP

Corporate MCP installer не устанавливает.

Registry существует, может быть пустым. Позже агент добавляет MCP profiles в процессе работы.

## 22. Memory Keeper

Memory Keeper subagent required by contract.

No fallback inside main chat.

CLI предоставляет команды `recall`, `reflect`, `teach`, `remember`, но смысловой выбор делает агент/Memory Keeper.

## 23. Writes to memory

Memory writes только по user-trigger:

- remember;
- teach;
- create workflow/process;
- create tool;
- reflect.

No background enrichment.

## 24. Reflection

Reflection user-triggered only.

Low-risk changes auto-apply. High-risk changes draft.

Rust CLI не вызывает LLM API. Reflection semantic extraction делает агент/Memory Keeper.

CLI должен поддерживать:

- inventory reflected sessions;
- track which chats/sessions were reflected;
- store raw/sanitized reflection materials;
- accept structured proposals;
- apply low-risk proposal items;
- keep high-risk proposal items draft.

Storage decision for v0.1:

- reflection sessions are node-backed records plus events;
- reflection proposals are structured proposal nodes plus events;
- workspace settings use existing registry/settings nodes or current storage;
- separate `reflection_sessions`, `reflection_proposals`, and
  `workspace_settings` tables are out of scope until needed.

JSONL parser не универсализируется в v0.1. Агент может создать tooling под конкретную shell при первой reflection.

## 25. Assistant thinking policy

Нельзя сохранять raw hidden chain-of-thought.

Можно использовать только доступные локальные visible/saved данные:

- user messages;
- visible assistant messages;
- tool call names;
- tool call args summaries;
- tool errors;
- final answers;
- explicit progress updates;
- available reasoning summaries, если shell явно сохраняет их и они доступны.

Любой reasoning-like материал должен быть дистиллирован в:

- lesson;
- correction;
- failure_mode;
- decision;
- workflow update;
- tool_contract update.

## 26. Risk classes

Low-risk auto-apply:

- add correction node;
- add failure_mode node;
- add lesson node;
- add alias/tag/link;
- update helpful metadata;
- create draft workflow/tool;
- update non-policy summary;
- add raw_note;
- add reflection_observation.

High-risk draft:

- kernel;
- gates;
- source hierarchy;
- security/secrets;
- external write policy;
- active workflow body rewrite;
- active tool replacement;
- deprecating active node;
- deleting/pruning knowledge.

## 27. Approval

Any message containing `+++` is approval.

External write / high-risk external action requires `+++`.

Internal AOPMem writes inside user-triggered modes do not require repeated approval.

External read does not require approval.

## 28. Hunch

Hunch enabled in v0.1.

Memory Keeper adds 1–3 hunches to recall bundle.

Hunch selection:

- FTS match;
- linked workflow/tool/failure_mode;
- hotness;
- no LLM.

Every hunch must have source node.

Hunch is not source of truth.

## 29. Generated CLI tools

Generated tools live under workspace tools directory.

Canonical registry: SQLite.

Local contract export: `tool.json` near tool implementation.

Tool creation only:

- by user request;
- or agent proposes, user accepts.

Memory Keeper can create draft tool only.

Generated tool tests are not required.

Mandatory contract:

- `tool.json`;
- `--help`;
- `--json`;
- `--dry-run` if side effects;
- strict args;
- clear errors;
- stable exit codes;
- examples.

Agent should call generated tools only through:

```text
aopmem tool run <tool-id> ...
```

## 30. Artifacts

Artifacts are temporary files produced by tools/reflection/reports/context bundles.

Stored only as files:

```text
~/.aopmem/workspaces/<workspace>/artifacts/YYYY-MM-DD/
```

Retention:

- max 7 days;
- max 1 GB per workspace;
- whichever comes first.

Cleanup must only delete files under `artifacts/`.

## 31. Audit git

Local audit git exists inside workspace.

It commits SQLite dump `.sql` / audit snapshots, not the binary SQLite DB.

No backup policy beyond SQL dump in v0.1.

## 32. Current state/task history

Out of scope.

No current_state. No task history memory.

## 33. QA domain pack

Out of scope.

## 34. PR/handoff contract

Out of scope.

## 35. Communication style

Default communication style is auto-seeded.

Installer does not ask a long configuration. It can briefly state that default style is installed and can be adjusted later.

## 36. Token/tool efficiency

Required core contract.

Agent must use Memory Keeper and AOPMem CLI, not raw memory/file digging.

## 37. Boy Scout rule for memory

Allowed only for low-risk metadata/link fixes.

Forbidden for body, policy, gates, kernel, security, source hierarchy.

## 38. Least privilege

Required for tools/MCP profiles.

Every tool/MCP profile must classify side effects.

## 39. Install questions

Installer silently determines technical details.

It must not ask or report technical facts it can know itself.

It asks only semantic/user knowledge:

1. Enable Understand Anything?
2. Enable Codebase Memory MCP?
3. Explain project purpose and what we do here.
4. Explain user role and agent role.
5. Explain working scope / auxiliary parts / forbidden areas.

No final confirmation ceremony.

## 40. LocalGitAudit

Enabled by default.

Does not touch working repo git.

## 41. Migration

Out of scope.

Clean init only.

## 42. Verification principles for development

Development must follow:

- BDUF light;
- KISS;
- YAGNI;
- thin slice;
- proof first;
- least surprise;
- self-documenting code;
- fail fast;
- separation of concerns;
- Boy Scout rule;
- least privilege;
- SOLID where useful;
- Occam's razor;
- Definition of Done;
- risk-based testing;
- regression safety;
- drift check;
- reproducible proof;
- negative testing;
- fast feedback.
