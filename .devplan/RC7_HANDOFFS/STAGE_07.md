# RC7 Stage 07 Handoff

Status: `VERIFIED`

Final full regression and all 20 audit sweeps passed. One initial P2 in
stream-exception unwrapping was fixed. The clean repeat reports:

```text
P1=0
P2=0
```

Exact Stage 07 changed files:

- `install/v0.2/install.ps1`;
- `scripts/test_windows_installer_transport.py`;
- `scripts/audit_v020_installers.sh`;
- `install/v0.2/install_prompt.md`;
- `docs/UPGRADE_TO_RC7.md`;
- `docs/WINDOWS_PROXY_INSTALL.md`;
- `.devplan/RC7_ASSET_REPORT.md`;
- `.devplan/RC7_PROXY_REDIRECT_SPEC.md`;
- `.devplan/RC7_SOURCE_CLASSIFICATION_REPORT.md`;
- `.devplan/RC7_WINDOWS_ACCEPTANCE_PROMPT.md`;
- `.devplan/GITHUB_RELEASE_NOTES_v0.2.0-rc7.md`;
- `.devplan/RELEASE_CANDIDATE_v0.2.0-rc7.md`;
- `.devplan/RC7_GLOBAL_AUDIT_REPORT.md`;
- `.devplan/RC7_CURRENT_STAGE.md`;
- `.devplan/RC7_EXECUTION_LEDGER.json`;
- `.devplan/RC7_PROOF_LOG.md`;
- `.devplan/RC7_HANDOFFS/STAGE_07.md`.

Native Windows runtime remains pending. Proceed to the one local release
commit. Keep the preserved untracked RC6 publication report outside it.
