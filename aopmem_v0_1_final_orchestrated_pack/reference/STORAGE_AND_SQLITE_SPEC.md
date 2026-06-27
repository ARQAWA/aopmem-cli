# STORAGE AND SQLITE SPEC

## Global path

Default:

```text
~/.aopmem/
```

Override allowed:

```text
AOPMEM_HOME=/custom/path
```

## Global structure

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

## Workspace key

```text
<sanitized-repo-folder-name>-<8-char-path-hash>
```

Hash input: absolute repo root path.

User does not input project id.

## SQLite storage

Each workspace has its own SQLite DB:

```text
~/.aopmem/workspaces/<workspace-key>/aopmem.sqlite
```

## Required SQLite pragmas

On every connection:

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
```

## Required schema groups

- `schema_migrations`
- `nodes`
- `links`
- `aliases`
- `tags`
- `sources`
- `events`
- `registries`
- `tool_contracts`
- `mcp_profiles`
- `fts_nodes`

## v0.1 node-backed records

The following concepts are implemented in v0.1 through existing nodes,
links, events, registries, and settings-like node records. Separate tables are
out of scope unless a later version needs them:

- `reflection_sessions` are represented by strict reflection record nodes and
  events.
- `reflection_proposals` are represented by structured `reflection_proposal_v1`
  nodes and apply receipt events.
- `workspace_settings` are represented by existing registry/settings nodes and
  current workspace storage mechanisms.

## Node types

Allowed node types:

```text
kernel_contract
gate
rule
workflow
skill
tool_contract
mcp_profile
project_profile
project_fact
decision
correction
lesson
failure_mode
incident_scar
preference
reflection_observation
raw_note
hunch_source
source
```

## Node statuses

Allowed statuses:

```text
draft
active
deprecated
superseded
broken
```

Deprecated/superseded nodes are excluded from normal recall.

## Trust/confidence

Every active node should have:

- `source_ref`
- `confidence`
- `trust_level`

Allowed source fallback:

```text
source=user_instruction
```

## FTS

FTS5 indexes:

- title;
- summary;
- body;
- aliases.

BM25 order: ascending score.

No semantic/vector search.

## Audit

Events go to SQLite `events`.

Audit-git commits SQL dump/snapshots, not binary DB.

## Artifacts

Artifacts are files only.

Path:

```text
~/.aopmem/workspaces/<workspace>/artifacts/YYYY-MM-DD/
```

Retention:

- 7 days;
- 1 GB per workspace;
- whichever comes first.

Cleanup deletes only under `artifacts/`.
