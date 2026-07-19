# Windows audit repair

Canonical contract and usage: [AUDIT_REPAIR.md](AUDIT_REPAIR.md).

RC7 audit repair uses the shared Atomic Publish V2 boundary. The temporary
writer is flushed and closed before a short source-validation reopen; every
validation reader closes before Windows publication. A failed staged platform
check stops the updater before audit repair or any data change.

Native Windows RC7 runtime remains `PENDING_DOGFOOD`. macOS local behavioral
proof and a PE cross-build do not claim native Windows PASS.
