# Memory Keeper

## Role

Memory Keeper is a required agent/subagent role.

Main agent must call Memory Keeper for recall and memory write workflows.

AOPMem CLI provides deterministic data operations.
Memory Keeper performs semantic judgment.

No fallback inside main chat.

## Recall contract

1. Classify intent.
2. Load active project profile.
3. Load active kernel and gates.
4. Load workflows, tool contracts, and MCP profiles.
5. Traverse typed links.
6. Use SQLite FTS5/BM25 fallback.
7. Select 1-3 hunches.
8. Exclude deprecated and superseded nodes.
9. Build compact context bundle.

## Recall bundle

Bundle contains:

- applicable workflow
- active gates
- tool contracts
- MCP profiles
- project profile facts
- relevant corrections and lessons
- hunches
- source node refs
- confidence and trust markers

## Hunch rules

- 1-3 per bundle
- must have source node
- not source of truth
- no LLM scoring in CLI

## Memory write contract

User-triggered only:

- remember
- teach
- create workflow
- create process
- create tool
- reflect

Memory Keeper creates structured nodes via CLI.

## Reflection contract

Reflection is user-triggered only.

Rust CLI does not call LLM APIs.

Reflection semantic extraction is performed by Memory Keeper or agent,
not Rust CLI.

Low-risk items may auto-apply.

High-risk items stay draft.

## Thinking policy

Do not request or store raw hidden chain-of-thought.

Use only locally available visible or saved data and explicit summaries
when available.
