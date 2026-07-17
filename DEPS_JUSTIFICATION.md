# Dependencies Justification

AOPMem v0.2.0-rc3 keeps one native Rust binary and a local-only runtime.
Every direct dependency in `Cargo.toml` is listed below. Release builds use
`Cargo.lock` through `cargo build --locked`.

## Cross-platform runtime dependencies

- crate: `clap`
  - Reason: typed parsing for the required CLI commands and global options.
  - Features: `derive` generates the parser without a runtime helper process.
- crate: `directories`
  - Reason: platform-correct user-level AOPMem home discovery.
- crate: `getrandom`
  - Reason: OS cryptographic randomness for local UUIDs, UI session tokens,
    export staging names, and durable backup/temp identifiers.
  - Boundary: reads the host random source; it performs no network request.
- crate: `gix`
  - Reason: create and verify real commits in the local audit Git repository
    without launching the `git` executable.
  - Features: defaults are disabled; `sha1`, `tree-editor`, and `zlib-rs`
    cover local object hashing, tree updates, and loose-object compression.
- crate: `rusqlite`
  - Reason: typed access to operational memory and local observability SQLite
    stores, including FTS5/BM25 retrieval and transactional migrations.
  - Features: `bundled` gives deterministic SQLite/FTS5 availability;
    `backup` enables consistent v0.1 database backups during upgrade;
    `functions` registers deterministic source-priority ordering for recall.
- crate: `serde`
  - Reason: typed serialization models for CLI, contracts, and local records.
  - Features: `derive` keeps model implementations explicit and local.
- crate: `serde_json`
  - Reason: stable CLI envelopes, tool contracts, reports, and redacted
    capsule records.
- crate: `thiserror`
  - Reason: typed internal errors without a broad application framework.
- crate: `tiny_http`
  - Reason: serve the embedded read-only desktop UI on one
    invocation-scoped IPv4 loopback listener.
  - Features: defaults are disabled; no TLS, async runtime, external asset,
    outbound HTTP client, frontend runtime, or daemon is added.
- crate: `uuid`
  - Reason: create and validate RFC 4122 UUID v4 values for bundle,
    correlation, event, artifact, and local audit identifiers.
  - Features: only `v4`; randomness comes from the local OS source.
- crate: `zip`
  - Reason: stream the required redacted debug capsule as a deterministic
    ZIP64 archive with fixed metadata and stored entries.
  - Features: defaults are disabled; export needs no compression, encryption,
    TLS, async runtime, or native library.

## Platform-specific runtime dependencies

- crate: `libc` (`macOS` only)
  - Reason: anchored `openat`/`renameat`/`fsync` file operations, safe process
    tree timeout cleanup, and local disk-space queries not exposed by `std`.
- crate: `windows-sys` (`Windows` only)
  - Reason: minimal Win32 bindings for anchored durable file operations,
    job-object/process-tree timeout cleanup, disk-space checks, and opening
    the local UI in the system browser.
  - Features: only the Foundation, Security, FileSystem, ToolHelp,
    JobObjects, Threading, Shell, and WindowsAndMessaging API groups used by
    the source.

## Runtime boundary

No dependency adds remote telemetry, cloud storage, a daemon, Node.js, or an
outbound application network path. `tiny_http` is server-only and the code
binds it to `127.0.0.1` with a per-run token. `gix` is used only against the
workspace-local audit repository. External processes exist only when the user
explicitly runs a generated tool under the bounded AOPMem tool runner.
