# TOOLS AND MCP REGISTRY

## Generated tools

Generated tools are created only:

- by direct user request;
- or after agent proposes and user accepts.

Memory Keeper creates draft tools only.

## Tool storage

```text
~/.aopmem/workspaces/<workspace>/tools/<tool-id>/
  tool.json
  bin/
  runtime/
```

## Canonical registry

SQLite registry is canonical.

`tool.json` is local contract/export near implementation.

## Tool contract

Every generated tool must have:

- stable id;
- command entrypoint;
- status;
- owner workflow;
- args schema;
- output schema;
- side effects;
- approval requirement;
- examples;
- runtime info.

## Tool invocation

Main agent must not call tool binaries directly.

Use:

```text
aopmem tool run <tool-id> --json -- <args>
```

## Tool validate

`aopmem tool validate <tool-id>` checks:

- tool exists;
- `tool.json` valid;
- executable path exists;
- mandatory fields exist;
- side effects class exists;
- examples exist.

No generated tool tests.

## Side effects enum

```text
none
local_read
local_write_artifact
local_write_memory
external_read
external_write
destructive
```

## MCP registry

Corporate MCP registry exists, may be empty.

AOPMem does not install corporate MCP in v0.1.

MCP profile fields:

- id;
- name;
- kind;
- status;
- read operations;
- write operations;
- side effects;
- approval requirement;
- credentials source;
- notes.

## Understand / Codebase

Installer may best-effort install/register:

- Understand Anything;
- Codebase Memory MCP.

Allowed optional MCP statuses:

```text
disabled
installed
missing
configured_unverified
```

Status meaning:

- `disabled`: user did not enable this optional MCP/tool.
- `installed`: CLI can verify local executable/capability using a
  deterministic detector.
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
