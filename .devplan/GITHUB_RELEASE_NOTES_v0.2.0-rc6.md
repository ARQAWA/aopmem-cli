# AOPMem v0.2.0-rc6

- Fixes Windows `ERROR_SHARING_VIOLATION` (`32`) during the staged platform
  check by closing the source writer before validation.
- Fixes the staged platform-check publication lifecycle while preserving
  no-replace, replace-existing, path, and final-validation safeguards.
- Preserves all RC5 functionality. The schema remains existing `004`; no new
  migration is included.
- Native Windows 11 / PowerShell 5.1 acceptance is still required after this
  prerelease. It is not claimed by macOS-hosted proof.

Verify downloads with the included `SHA256SUMS` manifest.
