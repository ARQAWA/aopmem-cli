# RC9 Windows transplant prompt

Use one native Windows PowerShell 5.1 chat.

```text
Goal: move RC4 user data into RC9 without in-place update.

Rules:
- no WSL;
- no Docker;
- no Cargo;
- no source build;
- no admin shell;
- no +++;
- do not delete quarantine;
- do not run old installer;
- do not edit old home except whole-home quarantine rename;
- stop on native evidence mismatch.

Inputs:
- old live home: %USERPROFILE%\.aopmem
- release: https://github.com/ARQAWA/aopmem-cli/releases/tag/v0.2.0-rc9
- asset base: https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc9
- installer: https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc9/install/v0.2/install.ps1
- harness:
  scripts/windows_rc4_to_rc9_transplant.ps1
  scripts/windows_rc4_to_rc9_transplant.py

Flow:
1. Run Action Plan.
2. Confirm AOPMem processes are absent.
3. Run Action Execute.
4. Harness quarantines the whole RC4 home.
5. Harness runs clean RC9 install into a fresh home.
6. Harness performs logical DB transplant.
7. Harness copies user tools, runtimes, artifacts, and secrets.
8. Harness rebuilds derived state.
9. Harness runs verification.
10. If any post-quarantine step fails, harness rolls back automatically.

Success result:
- report result = SUCCESS;
- binary reports aopmem 0.2.0-rc9;
- old quarantine retained;
- workspace identities preserved;
- row counts and semantic fingerprints match;
- secret paths preserved;
- doctor PASS;
- verify PASS;
- task smoke PASS.

Final answer must say native Windows PASS only after this really runs on
Windows.
```
