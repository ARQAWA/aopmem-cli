# RC8 Legacy Compatibility Report

Supported legacy evidence:

- RC7 orphan Safety Backup with no journal.
- RC7 target-version-coupled journals.
- Historical RC3-RC7 recovery names.
- Pending audit markers.

Policy:

- Orphan backup alone is stale pre-apply evidence.
- Malformed pre-apply journal can start fresh.
- Apply-started evidence blocks fresh run.
- Published evidence is historical.
- Installer Safety Backup is never normal adopt source.
