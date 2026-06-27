# PRODUCT SPEC — AOPMem v0.1

## Product definition

AOPMem is a local operational memory runtime for a trained agent-worker.

It is not a big agent OS, not a RAG system, not a memory SaaS, and not a code indexing tool.

It is a thin, strict CLI product that gives an agent:

- operational memory;
- workflows;
- gates;
- tool contracts;
- experience/corrections;
- reflection support;
- generated CLI tool routing;
- per-workspace storage;
- a deterministic recall interface.

## Why it exists

The original working prototype proved that an agent can become highly reliable through a local pack of rules, skills, tools, knowledge, reflection, and search. But that prototype grew organically. AOPMem v0.1 normalizes the pattern into a portable product.

## Product roles

### AOPMem

Stores operational memory and exposes it through a CLI.

### Memory Keeper

Agent/subagent role that calls AOPMem CLI, performs semantic interpretation, and returns clean context bundles.

### Main Worker Agent

Does the actual task. It does not manually dig through memory. It calls Memory Keeper.

### Understand Anything

Optional local project/docs understanding layer. If enabled, it receives `.understand.docs`.

### Codebase Memory MCP

Optional code navigation layer. Registered for coding tasks.

### Corporate MCP

External systems. Registry only in v0.1.

## Three contexts

### 1. Dev repo

Where AOPMem is built.

Contains Rust code, tests, proof, stage ledger, install prompt, docs, and build output.

### 2. Global host install

Where AOPMem lives on the user's machine:

```text
~/.aopmem/
```

### 3. Workspace init

AOPMem connects to a project/repo by creating a workspace under `~/.aopmem/workspaces` and inserting a managed block into the current agent instruction file.

## v0.1 target outcome

A user can:

1. Give an agent a GitHub install prompt.
2. Install AOPMem globally on macOS ARM.
3. Initialize a workspace for a repo.
4. Optionally enable Understand Anything and Codebase Memory MCP.
5. Give semantic project onboarding.
6. Start tasks with Memory Keeper recall.
7. Teach or remember new rules/processes.
8. Run reflection over sessions through agent-driven workflow.
9. Create draft generated CLI tool contracts.
10. Verify runtime health and artifacts cleanup.
