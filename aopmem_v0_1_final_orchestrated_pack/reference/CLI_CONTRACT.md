# CLI CONTRACT — MCP-like behavior

## Command name

Binary name:

```text
aopmem
```

## Core command groups

```text
aopmem init
aopmem status
aopmem doctor
aopmem verify

aopmem node create
aopmem node get
aopmem node list
aopmem node update

aopmem link add
aopmem link list

aopmem recall

aopmem remember

aopmem teach start
aopmem teach add
aopmem teach propose
aopmem teach apply

aopmem reflect inventory
aopmem reflect proposal create
aopmem reflect proposal apply

aopmem tool create-draft
aopmem tool list
aopmem tool get
aopmem tool run
aopmem tool validate

aopmem mcp list
aopmem mcp add
aopmem mcp get

aopmem adapter seed
aopmem adapter sync
aopmem adapter status

aopmem artifacts cleanup
```

## Output envelope

JSON mode returns:

```json
{
  "ok": true,
  "command": "recall",
  "data": {},
  "warnings": [],
  "errors": [],
  "meta": {
    "version": "0.1.0",
    "workspace_key": "example-a1b2c3d4"
  }
}
```

On error:

```json
{
  "ok": false,
  "command": "node_create",
  "data": null,
  "warnings": [],
  "errors": [
    {
      "code": "VALIDATION_ERROR",
      "message": "missing required field: type",
      "fix_hint": "provide --type with an allowed node type"
    }
  ],
  "meta": {
    "version": "0.1.0"
  }
}
```

## Exit codes

```text
0 success
1 generic error
2 invalid args
3 workspace not found
4 db/schema error
5 validation failed
6 unsafe action blocked
7 not implemented/out of scope
8 drift detected
9 io error
```

## Strictness

- No direct SQL access for agents.
- All machine reads should use `--json`.
- Errors must be clear and actionable.
- Do not print noisy logs to stdout in JSON mode.

## Approval

Any message containing `+++` is approval. CLI commands that require approval can accept:

```text
--approved +++
```

External write/high-risk external action requires approval.
