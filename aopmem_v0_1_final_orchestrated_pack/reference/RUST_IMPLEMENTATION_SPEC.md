# RUST IMPLEMENTATION SPEC

## Principles

Code implementation must follow:

- least surprise;
- self-documenting code;
- fail fast;
- separation of concerns;
- Boy Scout rule;
- least privilege;
- SOLID where useful;
- Occam's razor.

Planning must follow:

- BDUF light;
- KISS;
- YAGNI;
- thin slice;
- proof first.

Verification must follow:

- Definition of Done;
- risk-based testing;
- regression safety;
- drift check;
- reproducible proof;
- negative testing;
- fast feedback.

## Module boundaries

### `cli`

Command parsing, output envelope, exit codes.

No DB logic.

### `storage`

Workspace path resolution, SQLite connection, transactions.

No user-facing CLI formatting.

### `schema`

Migrations and schema checks.

### `recall`

Structured retrieval, links traversal, FTS fallback, hunch selection, bundle shaping.

No direct printing.

### `install`

Global install and workspace init logic.

Silent technical detection. Semantic questions only.

### `tools`

Tool registry, `tool run`, tool validation, draft tool creation.

### `reflection`

Reflection session inventory, proposal storage, apply low-risk/high-risk policy.

No LLM API.

### `verify`

Doctor/runtime checks and dev verification helpers.

### `audit`

Events, SQL dump snapshots, local audit git support.

### `artifacts`

Artifact paths and cleanup policy.

### `adapter`

Managed block generation/sync/status.

## Fail fast

On invalid input, return a structured error. Do not silently infer missing required values.

## No panic policy

No panics in normal CLI paths. Return errors with code/message/fix_hint.

Panics allowed only for programmer bugs in tests.

## CLI output

Human mode may be readable.

Machine mode must be JSON with stable envelope.

All proof tests should use JSON mode.
