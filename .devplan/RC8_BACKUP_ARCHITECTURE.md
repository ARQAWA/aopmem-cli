# RC8 Backup Architecture

RC8 splits backup roles.

| Backup | Owner | Purpose | Normal Adopt |
| --- | --- | --- | --- |
| Safety Backup | installer | emergency whole-home evidence | no |
| Upgrade Recovery Backup | verified RC8 binary | transactional recovery state | n/a |

Normal update creates Safety Backup first, then the RC8 binary creates a fresh
Upgrade Recovery Backup and journal.

Safety Backup may contain historical backup material. Upgrade Recovery Backup
uses canonical inventory and excludes only explicit product ephemeral files.
