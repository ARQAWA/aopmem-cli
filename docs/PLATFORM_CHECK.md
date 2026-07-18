# Platform self-check

`aopmem platform check --json` proves the local regular-file publish
primitives before an installer changes user data.

The command is intentionally independent from AOPMem state. It does not
resolve `AOPMEM_HOME`, inspect the current repository, create a workspace,
open operational or observability SQLite, or record an event. It needs no
administrator rights.

## Checks

The command anchors the operating system temporary root first, then creates
one exclusive UUID-named direct child. Unix uses fixed `/tmp`, ignores
`TMPDIR`, and creates the child with mode `0700`. Windows uses
`GetTempPathW`, then anchors that result. All write operations stay inside
the private child.

It proves:

1. regular source creation, write, and writable-handle flush;
2. `NoReplace` publication through `platform_publish`;
3. reopen and exact-byte validation;
4. rejection of a second `NoReplace`, with destination bytes unchanged;
5. `ReplaceOrCreate` over an existing destination;
6. reopen and exact-byte validation after replacement;
7. rejection of a non-direct-child destination;
8. anchored root identity and reparse guards;
9. the shared helper's fail-closed reparse contract, backed by its focused
   source/destination-link regression test;
10. bounded cleanup of known children and non-recursive removal of the empty
    private root.

Cleanup never uses `remove_dir_all`. It removes only known anchored children
and then calls non-recursive `remove_dir`. Failure output contains stable
roles and structured `raw_os_error`, I/O kind, publish phase, strategy, and
partial commit state. It never contains a raw temporary path or file content.

## JSON contract

Success includes:

```json
{
  "ok": true,
  "command": "platform_check",
  "data": {
    "schema_version": 1,
    "status": "pass",
    "location": "private_os_temp",
    "observability_recorded": false,
    "admin_required": false,
    "checks": [],
    "cleanup": {
      "attempted": true,
      "files_removed": true,
      "directory_empty": true,
      "root_removed": true
    }
  }
}
```

On failure, exit code is `9`. The update must stop before prepare, plan, or
apply. Structured details preserve the exact OS error, including Windows
error `87`, while `user_data_changed` and `observability_recorded` remain
`false`.

Native Windows execution is still `PENDING_DOGFOOD`. Stage 018 proves the
source contract and macOS runtime; it does not claim a native Windows pass.
