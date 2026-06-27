# DEV REPO SPEC

## Repository purpose

This repository is the product repository for AOPMem CLI.

It is not an installed AOPMem workspace.

## Required top-level structure

```text
aopmem/
  Cargo.toml
  README.md
  DEPS_JUSTIFICATION.md
  install/
    v0.1/
      install_prompt.md
  src/
    main.rs
    cli/
    storage/
    schema/
    recall/
    install/
    tools/
    reflection/
    verify/
    audit/
    artifacts/
    adapter/
  tests/
    cli/
    fixtures/
  scripts/
    dev_verify.sh
    build_macos_arm.sh
  docs/
    architecture.md
    cli.md
    storage.md
  .devplan/
    MASTER_SPEC.md
    FINAL_DECISION_LOG.md
    REQUIREMENTS_MATRIX.md
    EXECUTION_LEDGER.md
    EXECUTION_LEDGER.json
    STAGE_GRAPH.md
    CURRENT_STAGE.md
    BLOCKERS.md
    PROOF_LOG.md
    HANDOFFS/
  proof/
    commands.log
    outputs/
```

## One crate rule

The product must be one Rust crate named `aopmem`.

Use modules, not multiple crates.

## Local checks only

No GitHub Actions or CI in v0.1.

Required local checks:

- `cargo build`
- `cargo test`
- CLI proof scenarios
- negative scenarios
- drift checks

## Build output

v0.1 targets macOS ARM.

The repo must include a script or documented command to build the binary:

```text
scripts/build_macos_arm.sh
```

The exact binary output path must be deterministic, for example:

```text
dist/aopmem-darwin-arm64/aopmem
```

## Dependencies

Dependencies are allowed but must be recorded in `DEPS_JUSTIFICATION.md`.

For each dependency:

```text
crate:
version/range:
used for:
why not stdlib:
risk:
```

## No CI

Do not add `.github/workflows` in v0.1.
