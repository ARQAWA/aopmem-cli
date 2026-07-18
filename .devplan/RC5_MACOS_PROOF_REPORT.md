# RC5 Stage 24 — Native macOS Fresh and Mixed Update Proof

Status: `PASS`

Platform: `Darwin arm64`

Native Windows runtime: `PENDING_DOGFOOD`

Reproducer: `scripts/prove_rc5_macos.sh`

Retained proof root:

```text
/var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T//aopmem-rc5-stage24.q62Phs
```

No production code was changed by Stage 24. The stage adds one isolated
native proof harness and this evidence report.

## Result

The official RC5 macOS installer passed:

1. a fresh install with the exact five onboarding answers;
2. an update from one shared home containing two real workspaces:
   tagged v0.1 schema `001` and published active rc4 schema `003`;
3. repair of a genuine rc4 pending audit snapshot;
4. migration of both workspaces to operational schema `004`;
5. migration/creation of both Local Observability stores at schema v2;
6. exact CLI-visible node, node-alias, node-tag, and node-source
   preservation;
7. exact tool-tree byte preservation;
8. exact pre-update SQLite database/sidecar bytes in the durable full-home
   backup;
9. update of exactly the selected Codex adapter while preserving user text;
10. a pre-download failure that preserved the old binary and both databases
    while retaining its new backup.

The update asked no onboarding questions and invoked core apply exactly once.
No backup was deleted.

## Published source binaries

| Source | Version output | SHA-256 | Provenance |
|---|---|---|---|
| `v0.1.0-rc3` | `aopmem 0.1.0` | `d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607` | exact tagged Darwin asset |
| `v0.2.0-rc4` | `aopmem 0.2.0-rc4` | `4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e` | exact GitHub Release Darwin asset |
| RC5 candidate | `aopmem 0.2.0-rc5` | `245a7efff79119da59f955d4ee489f78321b90e03235512b432181cf4c8feb97` | prebuilt worktree candidate used only through installer test-asset injection |

The v0.1 workspace was initialized and populated only by the exact tagged
v0.1 binary. Its migration catalog ends at `001`. The second workspace was
initialized and populated only after activating the exact published rc4
binary. Its migration catalog ends at `003`. The proof never opened SQLite
directly.

## Fresh install

The fresh isolated home and repository started empty. The official installer
was invoked with an explicit exact pair:

```text
AOPMEM_ACTIVE_ADAPTER=codex
AOPMEM_ACTIVE_INSTRUCTION_FILE=AGENTS.md
```

It consumed the existing five questions, once each and in their product order.
The trace was:

```text
adapter.selected.codex
asset.download.started
sha256.verified
binary.version.verified
replacement.staged
replacement.published
init
adapter.seed
adapter.status
doctor
verify
task.start.smoke
observe.status
observe.report
debug.capsule.export
```

The installed binary hash equaled the verified candidate hash. `AGENTS.md`
contained exactly one `AOPMEM CONTRACT VERSION: 2`. Doctor, verify,
task-start, observability status/report, and debug export all passed.

## Mixed shared-home source

The update source had:

| Workspace | Source | Source schema | Canary |
|---|---|---:|---|
| `repo-schema001-6843b415` | tagged v0.1 | `001` | active rule, node alias, tag, source, validated tool tree |
| `repo-schema003-6643b0ef` | published rc4 | `003` | active workflow, node alias, tool tree, active v1 managed adapter |

Both workspaces used the same isolated `AOPMEM_HOME`. The installed active
binary was the exact published rc4 asset.

## Genuine pending repair

The harness did not create `.pending-snapshot` by hand. It temporarily made
the rc4 audit Git object directory unavailable, then executed a normal rc4
node mutation. The operational mutation committed and returned:

```text
AUDIT_SNAPSHOT_PENDING
```

The obstruction was restored before update. The pending marker existed in
the source and in the durable pre-download full-home backup. Staged official
`audit repair --all-workspaces --json` cleared the live marker. The
post-publish repair also passed.

## Official update order

The retained exact trace is:

```text
adapter.selected.codex
process.gate.clear
backup.created
backup.home.created
asset.download.started
sha256.verified
binary.version.verified
upgrade.backup.adopt
upgrade.stage
platform.check.staged
audit.repair.staged
upgrade.prepare
upgrade.plan
upgrade.apply
upgrade.apply.health.ok
upgrade.publish
adapter.sync
audit.repair.post-publish
adapter.status
doctor
verify
task.start.smoke
observe.status
observe.report
debug.capsule.export
```

Assertions proved:

- full-home backup completed before asset download;
- platform check completed before prepare;
- prepare completed before read-only plan;
- plan completed before apply;
- `upgrade.apply` appeared exactly once;
- publish happened only after apply;
- adapter sync happened only after publish;
- no `init` or `adapter.seed` appeared;
- the update read no onboarding input because stdin was `/dev/null`.

## Data and byte preservation

The harness used bounded CLI lists before and after update. It normalized only
the stable logical fields of Stage 24 canaries. Exact comparisons passed for:

- schema001 rule fields;
- schema003 workflow and pending-repair rule fields;
- both node-alias lists;
- the schema001 node-tag list;
- the schema001 secondary node-source list;
- both complete tool filesystem trees.

Timestamps were excluded from the logical projection. Tool files were checked
by sorted relative path plus SHA-256.

The schema003 canary did not seed a separate node-source row. Its node
`source_ref` was already included in the exact normalized node projection.

Before update, the harness hashed each present:

```text
aopmem.sqlite
aopmem.sqlite-wal
aopmem.sqlite-shm
```

for both workspaces. The same sorted relative-path manifest generated from the
installer's durable full-home backup matched byte-for-byte. This proves the
backup captured exact source database and coordination-sidecar bytes before
prepare and migration.

The old binary backup and full-home backup both retained the exact published
rc4 hash.

## Schema and observability result

Normal CLI health calls may create ordinary SQLite coordination sidecars. The
proof therefore used the official command:

```text
aopmem upgrade prepare --all-workspaces --json
```

It did not delete WAL/SHM manually. The following read-only plan returned:

```json
{
  "ready": true,
  "writes_performed": false,
  "workspaces": [
    {"current": "004", "target": "004", "pending": []},
    {"current": "004", "target": "004", "pending": []}
  ]
}
```

Both workspace observability status reports returned:

```text
observability_schema_version=2
collection_status=ready
```

Both verify commands were clean. Both task-start receipts had complete
mandatory and retrieval context. Both debug capsule exports succeeded.

The active rc4 workspace doctor was healthy. The non-selected schema001
workspace had DB, schema, audit, and tools `ready`, but its adapter status was
`missing`. This is expected and required: only the explicitly selected current
adapter may change. Its verify, task-start, observability, and export all
passed.

Source schemas `001` and `003` cannot contain migration-004 `tool_aliases`.
Stage 24 therefore proves preservation of their existing node aliases.
Schema004 tool-alias preservation remains covered by the Stage 21 focused
upgrade suite.

## Independent review remediation

An independent review found one P2 evidence gap: the schema001 tag and
secondary source were seeded, but the first proof version did not compare
their CLI projections before and after update.

The harness now runs bounded `tag list` and `source list` commands for the
seeded schema001 node, normalizes stable fields, and requires exact pre/post
matches. A new clean full run passed with `node_tags_preserved` and
`node_sources_preserved` both `true`. No manual SQLite access was added.

## Adapter isolation

The selected rc4 `AGENTS.md` changed from managed contract v1 to v2. It:

- retained `STAGE24 USER TEXT MUST SURVIVE` outside markers;
- contained exactly one v2 contract;
- had no duplicated managed block.

SHA-256 was unchanged for:

- `CLAUDE.md`;
- `.cursor/rules/aopmem.mdc`;
- `.github/copilot-instructions.md`;
- unrelated `OTHER.txt`.

## Failure safety

The failure case started from an exact copy of the retained pre-update home.
The installer was stopped by its deterministic `after_backup` injection.

Trace:

```text
adapter.selected.codex
process.gate.clear
backup.created
backup.home.created
rollback.unchanged
```

Assertions proved:

- no asset download;
- no prepare, plan, apply, or publish;
- installed rc4 binary bytes unchanged;
- both operational database bytes unchanged;
- current adapter bytes unchanged;
- old binary backup retained;
- durable full-home backup retained;
- failure text reported that the old binary was unchanged.

## Prohibited methods

The proof used none of:

- direct or manual SQLite;
- manual WAL/SHM deletion;
- manual pending-marker deletion;
- administrator rights;
- WSL;
- source build as an install workaround;
- Codex CLI launch.

The RC5 candidate was already built. It was supplied only through the
installer's explicit local test-asset boundary.

## Reproducible command

```text
cargo build --locked
AOPMEM_RC5_PROOF_BINARY="$PWD/target/debug/aopmem" \
  sh scripts/prove_rc5_macos.sh
```

Result:

```text
RC5 macOS Stage 24 proof: PASS
```

The harness intentionally retains the isolated proof root and every backup.

## Evidence integrity

```text
summary.json
0c0f36b0e862f738242df0add5161243381492d588f96404c19db9a70f7dcb33

update/update-trace.log
4c75363c45e4e1f4526a0d5023fb713e04b3b33e183d93f6976c3875f60c0620

fresh/trace.log
7a8f07e19bcc7b1f65518bf0c71550de24d899918ac754efb6030898d96ce4bc

failure-before-download/trace.log
faadda1fea89f16e7b9f8374f6d40133935def405a20f7c1ce8facc683196af1
```

## Review

Self-review: `PASS`.

Open P1: `0`.

Open P2: `0`.

Native Windows runtime remains `PENDING_DOGFOOD`.
