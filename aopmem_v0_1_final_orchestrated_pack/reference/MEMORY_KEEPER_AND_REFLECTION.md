# MEMORY KEEPER, RECALL, HUNCH, REFLECTION

## Memory Keeper

Memory Keeper is a required agent/subagent role.

Main agent must call Memory Keeper for recall and memory write workflows.

AOPMem CLI provides deterministic data operations. Memory Keeper performs semantic judgment.

## Recall algorithm

1. Classify intent.
2. Load active project profile.
3. Load active kernel/gates.
4. Structured lookup of workflows/tools/MCP profiles.
5. Graph traversal through links.
6. FTS/BM25 fallback.
7. Select 1–3 hunches.
8. Exclude deprecated/superseded nodes.
9. Build compact context bundle.

## Recall bundle

Bundle contains:

- applicable workflow;
- active gates;
- tool contracts;
- MCP profiles;
- project profile facts;
- relevant corrections/lessons;
- hunches;
- source node refs;
- confidence/trust markers.

## Hunch

A hunch is a short memory-derived warning/hint.

Rules:

- 1–3 per bundle.
- Must have source node.
- Selected by FTS match + linked workflow/tool/failure_mode + hotness.
- No LLM scoring in CLI.
- Not source of truth.

## Teach/remember

User-triggered only.

Commands/workflows:

- remember;
- teach;
- create workflow;
- create process;
- create tool.

Memory Keeper creates structured nodes via CLI.

## Reflection

User-triggered only.

No background daemon.

Reflection semantic extraction is performed by Memory Keeper/agent, not Rust CLI.

AOPMem CLI supports:

- session inventory;
- reflected session tracking;
- raw/sanitized materials storage;
- proposal storage;
- low-risk auto-apply;
- high-risk draft.

## Risk handling

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

## Assistant thinking policy

Do not request or store raw hidden chain-of-thought.

Use only locally available visible/saved data and explicit summaries if available.

Distill into operational artifacts.
