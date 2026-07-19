# RC6 Windows Acceptance Failure

Status: `RECORDED_EXACTLY; ROOT_CAUSE_NOT_YET_CLAIMED`

## Native environment

- Windows 11 Enterprise, build `22631`
- native Windows
- Windows PowerShell `5.1`
- no administrator rights
- corporate VDI

## Staged RC5 asset

- version: `aopmem 0.2.0-rc5`
- SHA-256:
  `150DB4699C2F41C6E529F9606AC099C9AC6B4771B5084952F2CB5DF3226D1B58`

## Command and result

```text
aopmem-windows-x86_64.exe platform check --json
```

- exit: `9`
- top-level code: `PLATFORM_CHECK_FAILED`
- operation: `no_replace_publish`
- nested code: `PLATFORM_PUBLISH_FAILED`
- phase: `validate_source`
- raw Windows error: `32`
- Windows meaning: `ERROR_SHARING_VIOLATION`
- source existed: `true`
- destination existed: `false`
- committed: `false`
- user_data_changed: `false`
- cleanup: `completed`

## Mandatory stop evidence

- audit repair not run
- upgrade prepare not run
- upgrade plan not run
- upgrade apply attempts: `0`
- migration not started
- binary publish not run
- active rc4 retained
- workspace DBs preserved
- repository unchanged

## Current real Windows installation

- version: `aopmem 0.2.0-rc4`
- SHA-256:
  `E4442FD06622A6B94F997E23B67A55753F1D841F6570EF20AC72B99083A6CC1C`
- workspaces:
  - `p-sit-cat-rental-8ef3bf83`
  - `p-sit-warranty-5708363a`

No antivirus, syscall, or handle-lifetime cause is asserted in this record.
