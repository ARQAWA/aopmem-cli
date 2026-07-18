# Tool Aliases and Deduplication

Status: RC5 Stage 015 local proof passed. The generic fixture, exact-only
apply, alias commands, and managed-block tool governance are implemented.

## Stage 015 Confluence fixture proof

`fixtures/stage_015/confluence_tools/` contains two bounded, secret-free,
valid contracts: `confluence_reader` and `confluence_reader_internal`.
Their runner bytes, behavior, runtime, launchers, and full fingerprint match;
only their ID and display name differ. Both start active. The fixture database
makes `_internal` older, proving the neutral-suffix rule selects
`confluence_reader` before the final created-at tie-break.

`tool dedupe plan --json` reports exactly
`SAME_IMPLEMENTATION_DIFFERENT_NAME`, `exact_only_eligible: true`, and the
canonical `confluence_reader`. `tool dedupe apply --exact-only --json` makes
the internal ID superseded in SQLite and manifest, adds its direct alias, and
keeps both source directories and runner bytes present. The old ID resolves
to the active canonical for get, validate, and run without `+++` because this
fixture is an `external_read` with approval `none`. Replay makes no new
canonicalization.

The same generic algorithm is also tested with a non-Confluence control pair.
No production branch or special case refers to Confluence. The factual
`tool.canonicalized` event contains only a bounded tool ID and approval flag;
it excludes arguments, output, paths, and raw contracts.

## Managed tool governance

The canonical Managed Block V2 requires one capability to use one canonical
ID with display name, aliases, and in-contract launchers. It forbids
user/internal/platform/short/wrapper copies. Before creation it requires
registry, alias, fingerprint, implementation, and description checks. Exact
duplicates return `TOOL_DUPLICATE` with no write; overlaps require review,
reuse, alias, or a real distinction. Creation needs a user request or an
agent proposal accepted by the user. There is no user-facing registry model.

Its approval rule remains unchanged: an `external_read` with approval `none`
needs no `+++`; external write, destructive, and high-risk actions require a
standalone exact `+++`.

## Direct alias model

`004_task_protocol_and_tool_aliases` adds `tool_aliases` to each operational
workspace database.

Each row contains:

- one workspace-unique `alias`;
- one direct `canonical_tool_id`;
- `created_at`;
- a bounded, non-empty `source`;
- the exact status `active`.

The target must be an existing active canonical row in `tool_contracts`.
An alias cannot target another alias. It cannot form a cycle or shadow a
non-superseded tool ID. A superseded old tool ID may become an alias to an
active canonical tool.

SQLite foreign keys, checks, indexes, and triggers preserve these invariants
even for writes below the Rust API. A canonical target cannot become
non-active while an alias points to it.

## Storage API

The typed Rust API supports:

- add, get, list, bounded keyset pages, bounded atomic bulk add, and remove;
- deterministic resolve;
- validation before writes;
- atomic rollback of a failed bulk;
- no filesystem side effects.

Resolution precedence is:

1. direct non-superseded tool;
2. active alias to an active canonical tool;
3. direct superseded fallback;
4. not found.

Alias rows are part of the operational revision fingerprint. Adding or
removing one invalidates stale task receipts.

## Filesystem boundary

Alias operations change SQLite only. They do not create or copy:

- tool directories;
- `tool.json`;
- executables;
- runtime or artifact namespaces.

## Alias-aware CLI

```text
aopmem tool alias add <alias> --to <tool-id>
aopmem tool alias list
aopmem tool alias remove <alias>
aopmem tool resolve <id-or-alias>
```

`tool get`, `tool validate`, and `tool run` resolve an alias before any
filesystem path is formed. Their manifest, executable, process cwd, and
artifact behavior therefore use only the canonical tool ID. Approval checks
are unchanged.

`tool get` reports the requested ID, matched alias, canonical ID, and
canonical contract. An alias run records the privacy-safe factual event
`tool.alias_resolved`.

`tool list` pages only non-superseded canonical registry records. Every
canonical row contains its aliases, loaded in one indexed batch query.
`--include-aliases` adds explicit deterministic alias rows, but aliases never
consume the canonical page limit or change its cursor.

## Canonical fingerprint

The fingerprint is SHA-256 with explicit domain and length separation over a
canonical JSON model. JSON object keys, safe relative paths, and platform
launcher order are normalized deterministically.

It excludes:

- tool ID and display name;
- status and timestamps;
- owner identity, examples, and cosmetic descriptions.

It includes:

- side effects and approval requirement;
- canonical input and output schemas;
- timeout, stdout/stderr limits, dry-run support, and output mode;
- command entrypoint, executable path, runtime layout, and platform launchers;
- relative implementation-file paths and SHA-256 hashes.

`platform_launchers` is a deterministic map inside the same `ToolContract`.
It defaults to empty when an older contract is decoded. Launcher names and
paths are bounded and validated. A launcher never creates a second contract
or tool directory.

The planner opens workspace/tool directories and files through OS-native
anchored no-follow handles. It validates the complete tree before reading
`tool.json`. It fails closed on contract drift, missing declared
implementation files, links/reparse points, path escape, non-regular files,
unreadable input, directory identity swap, mid-read drift, or a configured
resource bound.

## Read-only duplicate plan

```text
aopmem tool dedupe plan --json
```

The command opens a clean immutable SQLite snapshot and bypasses Local
Observability. It does not write SQLite, WAL/SHM, tool files, directory
metadata, or evidence. A present WAL/SHM sidecar fails closed rather than
reading a stale immutable view.

The stable JSON result contains:

- `writes_performed: false`;
- scan, shortlist, pair, and hashed-file counts;
- safe tool IDs;
- one of `EXACT_DUPLICATE`,
  `SAME_IMPLEMENTATION_DIFFERENT_NAME`,
  `SAME_CAPABILITY_DIFFERENT_WRAPPER`, `POSSIBLE_OVERLAP`, or `DISTINCT`;
- a separate `exact_only_eligible` boolean;
- bounded deterministic reason codes.

Raw canonical, capability, and implementation fingerprints are not exposed.
Exact-only eligibility is true only when full canonical fingerprints are
equal. A displayed class never grants eligibility by itself.

## Bounded work

The planner first builds deterministic capability, normalized-label, and
token indexes. It rejects any bucket that could exceed the pair cap before
generating its pairs. Only shortlisted tools reach filesystem validation and
hashing.

Hard bounds are:

- 1,000 registry tools;
- 10,000 candidate pairs;
- 256 implementation files per tool;
- 64 MiB implementation bytes per tool;
- 16 directory levels.
- 1,024 total descendant entries (files and directories) per implementation.

Filesystem and drift failures return the stable CLI reason code
`TOOL_DEDUPE_FILESYSTEM_UNSAFE`; raw paths are not an output surface.

Every shortlisted tool is scanned once. Every implementation file is hashed
once per operation and reused for all comparisons. Broad all-tools pairwise
comparison and normal-run implementation hashing are absent.

## Creation guard

`tool create-draft` performs a read-only registry preflight before draft
files or registry rows are written. It checks direct IDs, aliases, normalized
labels, behavioral capability facts, and a bounded in-memory BM25 search over
registry IDs and names.

An existing direct ID or alias collision returns `TOOL_DUPLICATE` with:

- canonical tool ID;
- alias suggestion;
- exact collision class;
- `writes_performed: false`.

A semantic candidate returns `TOOL_OVERLAP_REVIEW_REQUIRED`. The caller may
reuse the canonical tool, add an alias, or provide
`--technical-distinction`. The distinction is validated as non-blank UTF-8,
without NUL, at most 1024 bytes. Its raw text is never persisted, observed,
logged, or returned. Only the boolean fact that it was provided is returned.

A distinction bypasses only the current possible-overlap review. It never
bypasses a direct ID collision, alias collision, invalid contract, unsafe
path, symlink/reparse point, manifest drift, implementation drift, or a
resource bound.

Because a not-yet-created draft has no implementation bytes, the guard never
invents a proposed canonical fingerprint and never calls metadata equality
an exact duplicate. Shortlisted existing candidates are validated with the
same anchored Stage 012 implementation scanner and each file is hashed once
per preflight. The registry-only decision is repeated inside the coordinated
mutation before its first write.

Creation search bounds are:

- 1,000 registry tools;
- 64 shortlisted candidates;
- 512 search terms per document;
- 262,144 search terms total;
- the existing Stage 012 file, byte, and depth limits.

## Stage boundary

## Exact-only apply

`aopmem tool dedupe apply --exact-only --json` is the only apply form.
It rebuilds the bounded authoritative state inside one mutation lock and one
`BEGIN IMMEDIATE` transaction. Only equal full fingerprints are changed.

The active canonical is selected by active status, neutral suffix, shorter
ID, lexical ID, then older creation time. Duplicates become `superseded`,
their manifests are updated through no-follow handles, and direct aliases
resolve to the active canonical. Existing aliases targeting a duplicate are
retargeted first. Directories and executables are never copied or removed.

Exact fingerprint groups are grouped, not expanded into pairs. Non-exact
review candidates remain in the result. Failed operations roll back SQLite
and restore manifests through held anchored handles. A failed audit snapshot
does not undo a committed canonicalization.

Stage 013 does not canonicalize duplicates, change tool status, rewrite
contracts, delete directories, delete executables, or add the Confluence
fixture. Stage 014 implemented authoritative exact-only apply. Stage 015
completed the generic fixture and final managed-block tool rules.
