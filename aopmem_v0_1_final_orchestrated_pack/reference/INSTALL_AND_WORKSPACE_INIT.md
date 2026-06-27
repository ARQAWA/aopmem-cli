# INSTALL AND WORKSPACE INIT

## Core principle

Installer must follow KISS/YAGNI.

It must not ask or report technical facts it can determine silently.

## Silent technical detection

Installer silently determines:

- OS;
- repo root;
- current agent shell;
- target instruction file;
- existing managed block;
- existing global AOPMem;
- workspace key;
- backend path;
- whether `.understand.docs` exists.

It does not tell the user unless there is an error.

## Install flow

1. Check global AOPMem installation.
2. If missing, install global CLI to `~/.aopmem/bin`.
3. Create/verify global directories.
4. Create workspace under `~/.aopmem/workspaces/<workspace-key>`.
5. Ask whether to enable Understand Anything.
6. If enabled: best-effort install/register/index and create `.understand.docs`.
7. Ask whether to enable Codebase Memory MCP.
8. If enabled: best-effort install/register/index.
9. Ask semantic project onboarding.
10. Seed project profile into AOPMem from user answers.
11. Seed kernel/contracts/gates into SQLite.
12. Seed managed block into current instruction file.
13. Initialize audit-git and SQL dump snapshot.
14. Run `aopmem doctor`.
15. Run first recall bundle.

## User questions

Only these user-facing semantic blocks:

### 1. Understand Anything

```text
Включаем Understand Anything для локального понимания проекта и .understand.docs?
```

### 2. Codebase Memory MCP

```text
Включаем Codebase Memory MCP для навигации по коду?
```

### 3. Project meaning

```text
Объясни, что это за проект, зачем он нужен и чем мы тут занимаемся.
```

### 4. Roles

```text
Какая твоя роль в этом проекте и какая роль у агента?
```

### 5. Scope

```text
Какие части проекта рабочие, какие вспомогательные, какие нельзя трогать?
```

No final confirmation.

No technical questions.

## Communication style

Installer auto-seeds default communication style. It does not ask a long style questionnaire.

It may output briefly:

```text
Базовый стиль установлен. Его можно изменить позже через AOPMem.
```

## Managed adapter block

Insert only managed block. Do not overwrite full file.

## Optional MCP status

Install may register optional Understand Anything and Codebase Memory MCP
profiles with these statuses:

- `disabled`
- `installed`
- `missing`
- `configured_unverified`

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
