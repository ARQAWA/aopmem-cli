# RC7 Proof Log

## Stage 01 — baseline, field failure, worktree classification

Status: `VERIFIED`

### Baseline

```text
git status --short
  ?? .devplan/RC6_RELEASE_PUBLICATION_REPORT.md
git branch --show-current
  main
git rev-parse HEAD
  b47ff96681c77aa18a4baa07e030fa2dc78eeb88
git rev-parse origin/main
  b47ff96681c77aa18a4baa07e030fa2dc78eeb88
git rev-parse v0.2.0-rc6^{}
  b47ff96681c77aa18a4baa07e030fa2dc78eeb88
git show v0.2.0-rc6:install/v0.2/install.ps1 | shasum -a 256
  7e911088375755e64006e7c229bf2c604865b6fb5313da8870fd13ab0af85185
```

The only pre-existing worktree item is the deliberately untracked RC6
post-publication report. It is preserved unchanged and excluded from RC7.
No reset, checkout, stash, amend, rebase, force-push, or tag movement occurred.

### Source and evidence review

Read the complete Windows and macOS installers, canonical install prompt,
installer audit, RC6 macOS proof, Windows update docs, RC6 upgrade guide,
all `.devplan/RC6_*` records, RC6 release candidate, Cargo manifests, and
release build scripts.

Search confirmed one Windows production download path:
`Save-HttpsAsset` uses `Invoke-WebRequest -MaximumRedirection 0`, then accesses
`$_.Exception.Response` directly under strict mode. No RC6 proxy parameters or
environment proxy resolver exist.

Published source hashes were derived from immutable tagged assets:

| Release | macOS arm64 | Windows x86-64 |
| --- | --- | --- |
| RC4 | `4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e` | `e4442fd06622a6b94f997e23b67a55753f1d841f6570ef20ac72b99083a6cc1c` |
| RC5 | `594bb9606bd7f971a0fb97b16916fe2a5da84096e8340a5885c36d7037dd1b5e` | `150db4699c2f41c6e529f9606ac099c9ac6b4771b5084952f2cb5df3226d1b58` |
| RC6 | `b933d921ae6ec68ce7e0f118de27fd7eabe9d1c42d715a0a6df8f2ec731cb949` | `8cd03fd00ffdaf505d7f31cd1c485fd15179823f84a78061b7bcfc00ee4fd4c7` |

### Changed files

- `.devplan/RC7_CURRENT_STAGE.md`
- `.devplan/RC7_EXECUTION_LEDGER.json`
- `.devplan/RC7_PROOF_LOG.md`
- `.devplan/RC7_WINDOWS_INSTALLER_FAILURE.md`
- `.devplan/RC7_HANDOFFS/STAGE_01.md`

### Result

`PASS`. RC6 runtime binary is healthy on native Windows. Current blocker is
only official Windows installer transport and source classification. Product
architecture and schema remain out of scope.

### Handoff

Proceed to Stage 02. Specify one PowerShell 5.1-compatible `HttpClient`
transport with safe proxy resolution, manual redirects, streaming,
no-overwrite publication, cleanup, and deterministic proof.

## Stage 02 — transport specification and proof harness

Status: `VERIFIED`

### Changed files

- `.devplan/RC7_PROXY_REDIRECT_SPEC.md`
- `scripts/test_windows_installer_transport.py`
- `.devplan/RC7_CURRENT_STAGE.md`
- `.devplan/RC7_EXECUTION_LEDGER.json`
- `.devplan/RC7_PROOF_LOG.md`
- `.devplan/RC7_HANDOFFS/STAGE_02.md`

### Commands

```text
command -v pwsh
  absent
command -v powershell
  absent
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test_windows_installer_transport.py
  30 cases; expected RC6 red state: 29 failures, 0 errors
git diff --check
  PASS
```

The pure model defines all 30 required proxy, redirect, file, and exception
cases. Static assertions bind each case to production `install.ps1`. Only the
pre-existing relative-302 behavior passes before implementation.

### Result

`PASS` for specification and proof-first harness. Production remains
intentionally red until Stage 03. No local PowerShell runtime exists, so the
Mac cannot claim native PowerShell 5.1 execution.

### Handoff

Implement the specified single `HttpClient` transport. Keep proxy credentials
on the proxy object only; keep redirects manual; use same-parent partial files
and no-overwrite publication; preserve original exceptions.

## Stage 03 — PowerShell installer implementation

Status: `VERIFIED`

### Changed files

- `install/v0.2/install.ps1`
- `.devplan/RC7_CURRENT_STAGE.md`
- `.devplan/RC7_EXECUTION_LEDGER.json`
- `.devplan/RC7_PROOF_LOG.md`
- `.devplan/RC7_HANDOFFS/STAGE_03.md`

### Implementation

- Added public `[Uri]$ProxyUri` and
  `[switch]$ProxyUseDefaultCredentials`.
- Implemented immutable precedence:
  explicit, uppercase/lowercase HTTPS, uppercase/lowercase HTTP, usable system
  proxy snapshot, direct.
- Default network credentials are assigned only to the selected proxy object
  after opt-in. Target credentials remain disabled.
- Replaced exception-driven `Invoke-WebRequest` redirects with
  `HttpClientHandler.AllowAutoRedirect=false` and
  `ResponseHeadersRead`.
- Implemented only 301/302/303/307/308, relative redirects, HTTPS/userinfo
  checks, release-origin/CDN boundary, visited loop detection, and limit 10.
- Implemented bounded streaming into a same-parent `CreateNew` partial,
  exact optional `Content-Length`, durable flush, close, no-overwrite move,
  and owned-partial cleanup.
- Removed all `Exception.Response` and `MaximumRedirection 0` access.
- PowerShell invocation wrappers are unwrapped narrowly so the original
  network exception type/message remain in the classified error and inner
  exception.

### Commands

```text
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test_windows_installer_transport.py
  PASS; 30/30
rg Exception.Response install/v0.2/install.ps1
  no matches
rg "MaximumRedirection 0" install/v0.2/install.ps1
  no matches
git diff --check
  PASS
```

### Result

`PASS`. Mac proof is static/model only because local `pwsh` and `powershell`
are absent. Native Windows PowerShell 5.1 remains pending.

### Handoff

Proceed to Stage 04. Add exact platform-specific published source hashes
without changing compatibility decisions or apply ordering. Update the
installer audit to reject the RC6 transport patterns and prove the RC7 ones.

## Stage 04 — known-source classification and cumulative audit

Status: `VERIFIED`

### Changed files

- `install/v0.2/install.ps1`
- `install/v0.2/install.sh`
- `scripts/audit_v020_installers.sh`
- `.devplan/RC7_SOURCE_CLASSIFICATION_REPORT.md`
- `.devplan/RC7_STAGE04_CUMULATIVE_AUDIT.md`
- `.devplan/RC7_CURRENT_STAGE.md`
- `.devplan/RC7_EXECUTION_LEDGER.json`
- `.devplan/RC7_PROOF_LOG.md`
- `.devplan/RC7_HANDOFFS/STAGE_04.md`

### Commands

```text
sh -n install/v0.2/install.sh
  PASS
sh -n scripts/audit_v020_installers.sh
  PASS
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test_windows_installer_transport.py
  PASS; 30/30
rtk scripts/audit_v020_installers.sh
  PASS; 14 groups
git diff --check
  PASS
```

### Result

Exact published platform hashes now classify 0.1.0 and RC1–RC6. RC4/RC5/RC6
exact sources emit no warning. A modified compatible RC4 emits
`NONCANONICAL_SOURCE_BINARY`, displays its real version/hash, continues
through plan, and invokes apply once. Unsupported versions remain blocked.

A fresh high-reasoning cumulative audit reviewed Stages 01–04:

```text
P1=0
P2=0
```

No proxy credential leak, redirect/streaming blocker, installer order change,
product drift, schema drift, or private proxy hostname was found.

### Handoff

Proceed to full local regression and isolated macOS proof. Native Windows
PowerShell 5.1 remains pending and must not be inferred from Mac results.

## Stage 05 — full regression and macOS proof

Status: `VERIFIED`.

### Commands

```text
cargo fmt --all -- --check
  PASS
rtk cargo clippy --all-targets --locked -- -D warnings
  PASS; no issues
rtk cargo build --locked
  PASS
rtk cargo test --locked
  PASS; 771
rtk cargo test --tests --locked
  PASS; 771
rtk scripts/dev_verify.sh
  PASS; 769 unit + 2 integration and CLI proof
rtk scripts/audit_v020_installers.sh
  PASS; 14 groups
git diff --check
  PASS
sh scripts/prove_rc7_macos.sh
  PASS; fresh RC7 and RC4/RC5/RC6 to RC7
  candidate SHA-256:
  2e0b73e6d7a433e25466954beeee78d5bf722bb6364811ee9077b6d63b0b5510
```

### Changed files

- `.devplan/RC7_CURRENT_STAGE.md`
- `.devplan/RC7_EXECUTION_LEDGER.json`
- `.devplan/RC7_PROOF_LOG.md`
- `.devplan/RC7_HANDOFFS/STAGE_05.md`
- `scripts/prove_rc7_macos.sh`
- `.devplan/RC7_MACOS_PROOF_REPORT.md`

### Result

All current Rust, integration, CLI, installer, transport, classification, and
isolated macOS checks pass. The RC7-versioned candidate completed fresh, RC4,
RC5, and RC6 isolated macOS proofs.

### Handoff

Proceed to Stage 06 docs and reproducible release assets. Keep native Windows
acceptance pending.

## Stage 06 — version, docs, acceptance prompt, release assets

Status: `VERIFIED`

### Commands

```text
scripts/build_macos_arm.sh
  PASS
scripts/build_windows_x64_from_macos.sh
  PASS; two unchanged-source builds, identical SHA-256
dist/aopmem-darwin-arm64 --version
  aopmem 0.2.0-rc7
shasum -a 256 -c dist/SHA256SUMS
  PASS
git diff --check
  PASS
```

### Result

RC7 version, recovery names, docs, release notes, proxy bootstrap, standalone
Windows acceptance prompt, and flat release assets are complete. All hash and
byte markers were replaced. Schema remains `004`; no migration `005` exists.
Native Windows runtime remains pending.

### Handoff

Proceed to Stage 07 final regression and independent global audit. Require
P1=0 and P2=0.

## Stage 07 — final regression and global audit

Status: `VERIFIED`

### Commands

```text
cargo fmt --all -- --check
  PASS
cargo clippy --all-targets --locked -- -D warnings
  PASS
cargo build --locked
  PASS
cargo test --locked
  PASS; 771
cargo test --tests --locked
  PASS; 771
scripts/dev_verify.sh
  PASS; 769 unit + 2 integration and CLI proof
scripts/audit_v020_installers.sh
  PASS; 30 transport cases, 14 groups
git diff --check
  PASS
shasum -a 256 -c dist/SHA256SUMS
  PASS
jq ledger validation
  PASS
forbidden drift scan
  PASS
```

### Result

The first pass found one P2 in streamed-body exception unwrapping. It was
fixed and covered by the transport harness. The complete clean pass then
reported P1=0 and P2=0. Native Windows runtime remains pending.

### Handoff

Proceed to the exact local RC7 commit. Exclude the preserved historical
`.devplan/RC6_RELEASE_PUBLICATION_REPORT.md`.

## Stage 08 — local release commit

Status: `VERIFIED`

### Commands

```text
git branch --show-current
  main
git rev-parse HEAD
  b47ff96681c77aa18a4baa07e030fa2dc78eeb88
git ls-remote origin refs/heads/main
  b47ff96681c77aa18a4baa07e030fa2dc78eeb88
local and remote tag v0.2.0-rc7 absence checks
  PASS
privacy, secret, marker, cache, scope, and binary scans
  PASS
```

### Changed files

- `.devplan/RC7_CURRENT_STAGE.md`;
- `.devplan/RC7_EXECUTION_LEDGER.json`;
- `.devplan/RC7_PROOF_LOG.md`;
- `.devplan/RC7_HANDOFFS/STAGE_08.md`;
- `.devplan/RELEASE_CANDIDATE_v0.2.0-rc7.md`.

### Result

The commit containing this record is the one local audited release commit with
message `release: AOPMem v0.2.0-rc7`. The preserved historical RC6
publication report remains untracked and excluded.

### Handoff

Emit the exact External Action Draft. Stop until standalone `+++`.
