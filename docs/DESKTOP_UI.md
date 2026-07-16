# AOPMem Desktop UI

## Scope

`aopmem ui` opens a local, read-only browser for the current AOPMem workspace.
It is a desktop inspection tool. It is not an editor, tool runner, or remote
service.

The UI:

- reads the operational memory and Local Observability stores;
- binds only to `127.0.0.1`;
- uses embedded HTML, CSS, and JavaScript;
- uses system fonts and the system light or dark preference;
- makes no external network request;
- has no CDN, Node.js runtime, or frontend build step;
- does not create or update memory, tools, MCP profiles, or observability.

## Start and stop

From a project with an initialized AOPMem workspace:

```text
aopmem ui
aopmem ui --no-open
aopmem ui --port 0
```

`--port 0` selects a free random port. It is also the default. `--no-open`
prints the local URL without opening the system browser.

The command owns the server process. The server remains available while the
command runs. Stop it with the normal terminal interrupt. Closing the browser
tab does not start a daemon and does not move the server to the background.

If browser launch fails, the command prints `UI_BROWSER_OPEN_FAILED` as a
warning and continues to serve the printed local URL.

## Local security model

The server accepts only exact IPv4 loopback connections on `127.0.0.1`. Every
invocation creates a new random session token. The token is the first URL path
segment and is required for assets and API reads.

The UI keeps the token in the current page URL. API calls derive their base
from that URL. The token is not copied into browser storage, cookies, page
content, error text, or console output.

Additional controls:

- GET-only route allowlist;
- authentication before route and method handling;
- no wildcard CORS header;
- strict Content Security Policy;
- no upload, directory, arbitrary SQL, or file endpoint;
- no write endpoint;
- no external link or external asset;
- `Cache-Control: no-store` and `Referrer-Policy: no-referrer`;
- fetch uses omitted credentials, no cache, and rejected redirects.

Treat the printed URL as temporary local access. A new invocation gets a new
token.

## Sections

### Overview

Shows the workspace key, product version, memory counts, tool and MCP counts,
latest recorded doctor and verify state, latest recall bundle, and latest
failure, timeout, or overflow events.

If Local Observability does not exist, Overview says `not collected`. It does
not create the store.

### Memory

Shows a stable node table with type, status, summary, source trust, and update
time. Filters support exact node type, exact status, and bounded text search.

The list never contains node bodies. Selecting a node makes a separate,
explicit read for its complete body and a bounded page of incoming and
outgoing links.

### Graph

Shows one bounded node page and its bounded edge set. Filters support type,
status, and an optional positive center node ID.

The layout is local and deterministic. It uses a sorted breadth-first layout,
not random placement or a force simulation. The selected center stays visible
on centered continuation pages. A duplicate center returned in the normal
node page is rendered only once.

Hard response bounds are:

- at most 200 rendered graph nodes in total, including center context;
- at most 500 edges.

The UI does not append pages into an unlimited graph. `Next` replaces the
current node page. If edges reach their hard bound, the UI tells the user to
refine filters or select a center.

### Activity

Shows safe Local Observability event metadata. Filters support exact event,
outcome, and command IDs. Selecting a bundle ID opens its recall summary and
bounded selected-node facts.

Activity does not show raw payload JSON, node bodies, task text, chat text,
tool output, environment variables, secrets, tokens, or cookies.

### Effectiveness

Shows the same fact-only report as `aopmem observe report --json`, including:

- recall failures, empty results, overflow, and continuation;
- FTS and graph traversal usage;
- selected node types and top selected workflows, tools, and failure modes;
- explicit useful, partial, and wrong feedback;
- tool success, failure, timeout, and repeated errors;
- reflection proposal state;
- adapter drift, pending audit, doctor, verify, cleanup, and MCP facts.

The UI does not calculate or display a product score.

### Tools / MCP

Shows registered tool and MCP summaries. It includes status, owner or kind,
side effects, read/write description, and approval requirement.

This section cannot validate, approve, run, install, update, or remove a tool
or MCP profile.

## Pagination contract

List reads use stable keyset pagination. The default page size is 100. The
server maximum is 500. Cursors are opaque and bound to their endpoint and
filter scope.

The browser keeps only the current cursor and a small in-memory Back stack.
It never parses or edits a cursor. Changing filters resets the stack.

Every page shows whether more results exist. An incomplete page without a
continuation cursor is treated as an invalid response. There is no silent
truncation and no automatic full traversal.

## UI states

Each section has explicit states:

- loading;
- ready;
- empty;
- partial or truncated;
- safe error;
- retry.

Changing section or retrying cancels the old request. A sequence guard ignores
a late response from a canceled request. Errors show only the bounded API code
and safe API message.

## Accessibility and desktop layout

The UI has a skip link, semantic navigation and tables, visible keyboard
focus, live status text, text status labels, and keyboard-selectable graph
nodes. Enter or Space opens a graph node. Escape closes the details panel.

The graph also provides a normal accessible node list. Color is not the only
status signal.

The minimum supported layout width is 1100 pixels. Mobile and tablet layouts
are outside the v0.2 release scope.

The stylesheet uses system fonts and `prefers-color-scheme`. It uses no remote
font. Reduced-motion preference disables any incidental browser transition or
animation.

## Privacy boundary

Memory node bodies are loaded only after an explicit node selection. They stay
inside the local authenticated browser session.

Local Observability views use the privacy-bounded read DTOs. They do not expose
the operational SQLite database, observability SQLite database, raw artifacts,
raw tool output, full task text, or hidden reasoning.

The UI itself is not recorded as an observability event. Opening and browsing
the UI leaves the main database bytes, schema, and row counts unchanged.

Both stores use SQLite WAL mode. A normal read-only SQLite connection may
create or touch transient `-wal` and `-shm` lock-coordination sidecars. The UI
does not write product data to those files and does not delete them. Treating a
live database as `immutable` would disable SQLite locking and change detection,
so the UI deliberately keeps the normal read-only WAL safety model.

## Release proof

The release proof uses a temporary AOPMem home and temporary workspace. It must
never open a real user workspace.

Required viewport captures are exactly 1440 by 900 pixels:

```text
.devplan/AUDITS/ui-overview-1440x900.png
.devplan/AUDITS/ui-graph-1440x900.png
.devplan/AUDITS/ui-activity-1440x900.png
```

The screenshots use a temporary workspace seeded through the CLI and the real
local API; no route interception is used. The same browser run verifies all six
sections. The proof also checks:

- no external request;
- no browser or page error;
- dark system preference through computed styles;
- exact PNG dimensions and SHA-256 hashes;
- unchanged main operational and observability SQLite bytes, size, mtime,
  schema, and row counts (WAL/SHM lock metadata is recorded separately);
- clean server shutdown and closed local port.

## Developer checks

Run the focused embedded asset and UI tests, then the normal Rust gates:

```text
cargo test ui::assets
cargo test ui::http
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build
```

The embedded asset tests reject external resources, browser storage, unsafe
HTML sinks, write methods, editing controls, and missing read-only API routes.
