Use this prompt to install and initialize AOPMem v0.1 for the current
repository.

```text
You are installing AOPMem v0.1 into the user's current macOS Apple Silicon
host and initializing it for the current repository.

Important rules:

- Do the full install and workspace init flow.
- Stay inside user-level install only.
- Detect technical facts silently.
- Do not ask the user about things you can detect yourself.
- Do not ask any irrelevant technical questionnaire.
- Ask only the 5 semantic questions listed below.
- No final confirmation ceremony.
- If the managed AOPMem block already exists, update only that block.
- If the managed block is damaged, stop with an explicit error.
- If the host is not macOS Apple Silicon, stop and explain that v0.1 supports
  only macOS ARM.

Silent technical detection:

- current OS and architecture
- current repo root
- current agent environment and instruction file
- existing managed AOPMem block
- whether AOPMem is already installed globally
- workspace key
- backend path under ~/.aopmem
- whether .understand.docs already exists

Install flow:

1. Check whether AOPMem is already installed globally.
2. If missing, install the AOPMem CLI into ~/.aopmem/bin.
3. Create and verify the required global directories under ~/.aopmem.
4. Create or reuse the workspace for the current repository under
   ~/.aopmem/workspaces/<workspace-key>.
5. Ask whether to enable Understand Anything.
6. If enabled, do best-effort local setup and create .understand.docs.
7. Ask whether to enable Codebase Memory MCP.
8. If enabled, do best-effort local setup.
9. Ask the semantic project onboarding questions.
10. Seed the collected semantic answers into AOPMem.
11. Seed the required default kernel/contracts/gates data.
12. Insert or update the managed AOPMem block in the current instruction file.
13. Initialize local audit snapshots.
14. Run `aopmem doctor`.
15. Run the first recall bundle.

Ask only these 5 user-facing questions, exactly in this order:

1. Включаем Understand Anything для локального понимания проекта и
   .understand.docs?
2. Включаем Codebase Memory MCP для навигации по коду?
3. Объясни, что это за проект, зачем он нужен и чем мы тут занимаемся.
4. Какая твоя роль в этом проекте и какая роль у агента?
5. Какие части проекта рабочие, какие вспомогательные, какие нельзя трогать?

Do not ask about:

- OS
- shell
- repo path
- workspace id
- database location
- adapter type
- instruction file name
- whether to use the default communication style
- any long preferences/style questionnaire
- any other technical facts you can detect silently

After install:

- briefly report what was done
- report any optional setup that was skipped or failed in best-effort mode
- confirm doctor result
- do not dump unnecessary technical detail unless there is an error
```
