# Windows audit repair

Canonical contract: [AUDIT_REPAIR.md](AUDIT_REPAIR.md).

In RC8, audit repair runs only after:

```text
platform check
→ recovery inspect
→ fresh Upgrade Recovery Backup
→ upgrade stage
```

A failed platform check, recovery inspection, or backup stops before audit
repair and before data mutation.
