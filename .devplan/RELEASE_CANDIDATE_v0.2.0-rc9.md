# AOPMem v0.2.0-rc9 release candidate

RC9 removes the штатный updater from the active product.

Architecture:

```text
quarantine old home
clean install
logical transplant
verify
retain quarantine
```

Product contract:

- legacy upgrade CLI is absent;
- in-place binary/home transition is not supported;
- installer is clean-only;
- existing populated `AOPMEM_HOME` fails closed with
  `CLEAN_INSTALL_REQUIRES_EMPTY_HOME`;
- old home is never auto-renamed or modified by installer;
- external RC4 to RC9 transplant harness is one-shot and outside the runtime.

Schema:

- current schema remains `004_task_protocol_and_tool_aliases`;
- no schema `005` is added.

Required assets:

- `dist/aopmem-darwin-arm64`;
- `dist/aopmem-windows-x86_64.exe`;
- `dist/SHA256SUMS`.

Release:

- tag: `v0.2.0-rc9`;
- title: `AOPMem v0.2.0-rc9`;
- prerelease: true.
