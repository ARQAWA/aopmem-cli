# RC5 Tool Deduplication Report

Status: `VERIFIED_THROUGH_STAGE_015`

## Stage 015 result

The deterministic fixture under `fixtures/stage_015/confluence_tools/` proves
the field Confluence case without a production special case. Both contracts
are active before apply and have identical executable bytes and complete
canonical fingerprints. `confluence_reader_internal` is deliberately older
in the fixture SQLite database. The neutral-suffix selection rule still picks
`confluence_reader`; created-at remains the final tie-break only.

Focused proof covers plan classification and eligibility, one exact-only
canonicalization, SQLite/manifest supersession, direct old-ID alias, canonical
default list, alias get/validate/run resolution, unchanged `external_read`
approval `none`, retained directories/runner bytes, idempotent replay, safe
`tool.canonicalized` observability, and a non-Confluence control pair.

## Stage 014 result

Exact-only apply is implemented. It rechecks the registry and filesystem
inside the mutation transaction, groups equal canonical fingerprints without
quadratic expansion, and only supersedes duplicates with an active canonical.
It preserves directories and executables, retargets direct aliases, restores
manifests through anchored handles on rollback, and emits safe post-mutation
canonicalization facts.

## Audit remediation

The scanner now has a hard 1,024 descendant-entry cap, streamed before
sorting or recursion. It rechecks anchored identity after each manifest and
implementation read. CLI filesystem failures use a stable safe reason code
and do not expose raw paths. The fresh independent cumulative audit accepted
the remediation with P1 `0` and P2 `0`.

## Stage 013 addition

Stage 013 adds the create-draft guard without weakening Stage 012 exact
eligibility.

A not-yet-created draft has no implementation bytes. The guard therefore
does not invent a proposed full fingerprint. Direct ID/alias collisions are
exact registry collisions. Semantic name/capability/BM25 candidates remain
`POSSIBLE_OVERLAP` until implementation evidence exists.

Shortlisted existing candidates reuse the Stage 012 anchored implementation
scanner. Each file is hashed once. A bounded technical distinction may bypass
only possible-overlap review and its raw text is never persisted, observed,
returned, or logged.

Stage 014 completed authoritative in-mutation exact-only apply.

## Stage 012 result

Stage 012 implements a canonical tool fingerprint and deterministic,
strictly read-only `aopmem tool dedupe plan --json`.

No apply, status transition, alias write, directory deletion, executable
deletion, alias-aware runner, creation guard, or Confluence-specific behavior
is present.

P1: `0`.

P2: `0`.

## Fingerprint contract

The full fingerprint uses SHA-256 with domain and length separation. It
canonicalizes JSON object keys, relative layout paths, and the
`BTreeMap`-backed platform launcher map.

Included facts:

- side effects;
- approval requirement;
- input and output schemas;
- timeout and output limits;
- dry-run and output-mode contracts;
- command/executable/runtime relative layout;
- platform launchers;
- every implementation relative path and file hash.

Excluded facts:

- tool ID;
- display name;
- status;
- created/updated timestamps;
- owner identity;
- examples and cosmetic descriptions.

The separate capability signature omits wrapper/layout/implementation facts
and is used only for bounded shortlisting and classification.

## Classification and eligibility

The public enum contains all five frozen classes:

1. `EXACT_DUPLICATE`;
2. `SAME_IMPLEMENTATION_DIFFERENT_NAME`;
3. `SAME_CAPABILITY_DIFFERENT_WRAPPER`;
4. `POSSIBLE_OVERLAP`;
5. `DISTINCT`.

Displayed class and exact-only eligibility are separate fields.

```text
exact_only_eligible =
    canonical_fingerprint(left) == canonical_fingerprint(right)
```

Therefore a pair may display
`SAME_IMPLEMENTATION_DIFFERENT_NAME` and still be exact-only eligible, as
required by RC5-D-028 and RC5-D-029.

The JSON API exposes IDs, class, eligibility, reason codes, and bounded
counts. It does not expose raw fingerprints.

## Deterministic canonical suggestion

The plan orders a pair by the frozen selection rules:

1. active before non-active;
2. neutral ID before platform/user/internal/wrapper suffix;
3. shorter ID;
4. lexical ID;
5. older `created_at` as final tie-break.

This is a suggestion only. Stage 012 performs no canonicalization.

## Complexity and bounds

The algorithm builds `BTreeMap` indexes for capability signatures,
normalized labels, and normalized tokens before any implementation hash.

Each bucket's theoretical pair count is checked before generation. The union
is deduplicated in a `BTreeSet`. Any overflow fails before filesystem hashing.

Hard limits:

| Resource | Bound |
|---|---:|
| registry tools | 1,000 |
| shortlisted pairs | 10,000 |
| files per implementation | 256 |
| bytes per implementation | 64 MiB |
| directory depth | 16 |

Shortlist/index work is `O(T log T)`. Pair work is bounded by the constant
10,000 cap. Filesystem work is `O(F + B)` only for shortlisted tools. Each
tool is scanned once and each file is hashed once per operation. Cached
results serve all comparisons. No normal tool run computes these hashes.

## Fail-closed filesystem contract

The planner:

- anchors workspace, tools root, tool directory, descendants, manifests, and
  implementation files with the existing OS-native no-follow handle layer;
- validates the entire descendant tree before reading `tool.json`;
- validates the real tool root under the real workspace tools directory;
- rejects symlinks and Windows reparse points;
- rejects path escape, non-UTF-8 relative paths, and special files;
- rejects missing declared entrypoint, executable, or platform launcher;
- rejects unreadable files;
- checks file size/mtime/type after hashing;
- rereads `tool.json` after hashing and rejects drift;
- revalidates the anchored tool-directory identity after hashing;
- enforces file, byte, depth, tool, and pair limits.

## Zero-write proof design

The CLI bypasses Local Observability and opens SQLite with
`mode=ro&immutable=1`, `query_only=ON`, and in-memory temp storage.

It first rejects existing WAL/SHM sidecars. This avoids both:

- creating a shared-memory sidecar during a nominal read;
- ignoring committed WAL state in an immutable view.

The Stage 012 test snapshots the complete AOPMem test home before and after
two plans. It compares:

- every path;
- file/directory kind;
- byte length;
- nanosecond mtime;
- SHA-256 file content.

The snapshots are exactly equal. The plan is identical across both runs.
No observability database is created.

## Focused proof

Implemented focused cases cover:

- all five classes;
- exact eligibility separated from class;
- schema object-key canonicalization;
- identity/status/time/cosmetic exclusion;
- launcher inclusion;
- legacy missing-launcher default;
- deterministic output ordering;
- safe JSON contract without raw fingerprint fields;
- one hash per implementation file;
- pair overflow before filesystem access;
- symlinked `tool.json` rejection before manifest read;
- symlink escape rejection;
- same-path tool-directory swap rejection;
- exact filesystem/SQLite/WAL/directory/mtime no-write proof.

Stage 013 will add alias-aware CLI behavior. Stage 014 will consume this typed
plan API for an authoritative in-mutation exact-only rescan and apply. Stage
015 will add the generic Confluence fixture with no product special case.
