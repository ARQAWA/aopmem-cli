# RC7 Stage 02 Handoff

Status: `VERIFIED`

The canonical design is `.devplan/RC7_PROXY_REDIRECT_SPEC.md`.
The executable contract is `scripts/test_windows_installer_transport.py` and
contains exactly 30 test methods.

RC6 expected red state: 29 failures, zero errors. The one passing case is
relative `302`, already present in RC6.

Implement only `install/v0.2/install.ps1` in Stage 03. Use PowerShell
5.1-compatible `System.Net.Http`, one immutable proxy configuration, manual
301/302/303/307/308 handling, loop/limit protection, same-parent partial
streaming, create-new/no-overwrite semantics, and original-error preservation.

Local `pwsh` and `powershell` are absent. Static/model proof is allowed; native
PowerShell 5.1 remains pending external Windows acceptance.
