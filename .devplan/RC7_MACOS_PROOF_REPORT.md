# RC7 macOS isolated proof

Status: `PASS`

Host requirement: Darwin arm64.

Command:

```sh
sh scripts/prove_rc7_macos.sh
```

Executed on 2026-07-19 against a candidate reporting
`aopmem 0.2.0-rc7`.

## Evidence

- proof root:
  `/var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T/aopmem-rc7-stage05.lFfAkK`;
- summary:
  `/var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T/aopmem-rc7-stage05.lFfAkK/summary.json`;
- candidate SHA-256:
  `2e0b73e6d7a433e25466954beeee78d5bf722bb6364811ee9077b6d63b0b5510`;
- RC4/RC5/RC6 update paths: `PASS`;
- exact backup byte checks: `PASS`;
- schema, repair, task lifecycle, debug export, and UI smoke: `PASS`;
- installer apply count: `1`;
- update onboarding questions: `0`;
- repository-local `.aopmem`: absent.

## Expected coverage

- local RC7 candidate installed through the official isolated test path;
- direct installer path with all common proxy environment variables removed;
- exact published RC4, RC5, and RC6 Darwin arm64 source assets;
- published `SHA256SUMS`, fixed SHA-256, and exact `--version` verification;
- fresh RC7 install;
- separate RC4-to-RC7, RC5-to-RC7, and RC6-to-RC7 source fixtures;
- exact source database, tool, and artifact bytes in the durable backup;
- logical data, tool files, and artifact files readable after update;
- all three workspaces at schema `004`;
- staged platform check, audit repair, prepare, plan, one apply, publish,
  adapter sync, post-publish repair, doctor, and verify;
- exactly one installer apply;
- zero update onboarding or adapter seed actions;
- task start, apply, and complete;
- debug capsule export;
- loopback `ui --no-open --port 0` HTML smoke with the read-only badge;
- no repository-local `.aopmem`;
- isolated temporary homes, repositories, assets, and output.

## Boundary

No Windows runtime claim. Native Windows acceptance remains separate.
