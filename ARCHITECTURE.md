# Architecture

This document records the initial architecture decision for the agent-aware
Zellij tab bar. It is both a design guide and a boundary: implementations may
change behind the interfaces described here, but the event model and user-facing
protocol should evolve compatibly.

## Status

Accepted for the initial implementation.

## Problem

Zellij knows about tabs, panes, focus, and working directories, while an agent
process knows whether it is idle, running, complete, or failed. Neither side has
enough information to render a useful agent-aware tab bar by itself.

The project must join those two event streams without becoming an agent runner,
terminal emulator, Git client, or cloud service. It must:

- derive concise tab names from pane directories;
- show agent state with badges that remain meaningful without color;
- preserve user-assigned tab names;
- remain correct when tabs move, panes close, or events arrive late;
- update from events rather than polling; and
- keep malformed external input from crashing the plugin.

## Constraints

### Zellij boundary

The plugin runs as WebAssembly inside Zellij. The CLI is a native process and
may be invoked by shell hooks, humans, or process wrappers. Communication must
use an interface available to both without requiring a daemon or network
listener. The plugin crate exposes its testable implementation as a Rust
library and uses a small WASI binary adapter to register it with Zellij. Release
builds verify the `_start` and plugin callback exports expected by the host.

### Stability

CLI-to-plugin messages cross a process and release boundary. Their encoding is a
public protocol, even though the Rust types and concrete trait implementations
remain private whenever possible. Additive changes must not break older peers,
and unsupported protocol versions must fail safely.

### Correctness

Tab positions are presentation details, not identities. State must be keyed by
stable tab and pane identifiers. Each externally reported status stream carries
a sequence number, and a reducer ignores a sequence number it has already seen
or passed. Status observations belong to a generation so an older run cannot
overwrite a newer run for the same pane and agent. Current observations
aggregate with this precedence:

1. `Running`
2. `Error`
3. `Complete`
4. `Idle`

### Reliability

External events, paths, configuration, and Zellij state are untrusted input.
Production paths return or log recoverable errors; they do not use `unwrap()` or
`expect()`. A bad message may be discarded, but it must not poison previously
valid state or panic the plugin.

### Performance

The design targets plugin startup below 100 ms, status propagation below 10 ms,
and memory use below 25 MB under ordinary interactive workloads. These are
budgets to measure, not reasons to add background work. The steady state has no
polling loop.

### Dependency direction

The Cargo workspace follows one-way dependencies:

```text
cli ───────► shared ◄─────── plugin
```

`shared` depends on neither executable crate. Cycles are prohibited.

## Alternatives Considered

### Poll a state file

The CLI could write a JSON file and the plugin could periodically reload it.
This is straightforward and survives plugin restarts, but it adds latency,
causes unnecessary wakeups, needs locking and cleanup, and violates the
zero-polling requirement. Persistence can be added independently later.

### Run a local daemon

A Unix socket service could centralize ordering and persistence. It also adds a
third lifecycle, installation and security concerns, platform-specific socket
handling, and failure modes that are disproportionate to a tab-bar plugin.

### Encode state in tab names

The CLI could rename tabs directly. This couples status to presentation, makes
manual-name detection unreliable, loses structured history, and causes multiple
writers to compete over one string.

### Call plugin internals from the CLI

Sharing implementation APIs would couple a native executable to a WASM plugin
and would not cross the process boundary. Shared domain types are useful;
shared runtime state is not.

### Zellij named pipes with versioned events

Zellij already routes named pipe messages to plugins and identifies the active
session. This avoids a daemon, socket, or polling loop. The tradeoff is that
delivery is transient and messages require validation at the plugin boundary.
This is the selected transport.

## Chosen Design

### Components

```text
zja CLI / wrapper
       │
       │ versioned JSON event
       ▼
Zellij named pipe: zja.events
       │
       ▼
Transport decoder ──────► diagnostic event log
       │
       ▼
State reducer ◄────────── Zellij tab/pane/focus events
       │
       ├────────► directory resolver
       │
       ▼
Immutable render model
       │
       ▼
Renderer ───────────────► Zellij tab-bar UI
```

The CLI publishes a protocol event by executing the equivalent of:

```console
zellij pipe --name zja.events -- '<json-event>'
```

The plugin subscribes to pipe messages and the Zellij tab, pane, focus, and
closure updates it needs. Receiving either kind of input triggers one bounded
reduction and, only when visible state changes, a render request.

The event log is diagnostic: it records accepted and rejected input at the
configured log level. It is not a source of truth or a promise of durable
persistence.

### Shared domain

`shared` owns the data and behavior that must agree across native and WASM
targets:

- protocol envelope and event payloads;
- agent state and aggregate precedence;
- reducer inputs and immutable snapshots;
- directory-name candidate and disambiguation rules;
- render-model types and Unicode-safe layout helpers; and
- configuration types and validation.

Serialization belongs at the transport boundary. Zellij host types and process
execution remain in their respective adapter crates.

### Interface boundaries

Implementations are hidden behind four small roles:

```rust
trait Renderer {}
trait StatusStore {}
trait DirectoryResolver {}
trait Transport {}
```

These names describe substitution seams, not mandatory public traits. A trait is
made public only when another crate must implement it. The intended
responsibilities are:

- `Transport`: encode, deliver, decode, and validate event envelopes;
- `StatusStore`: apply accepted events and expose immutable snapshots;
- `DirectoryResolver`: derive automatic display names from pane/tab context;
- `Renderer`: turn a snapshot and available width into styled output.

Keeping reduction pure makes event order, stale rejection, naming, and rendering
testable without running Zellij.

### Event flow

All state changes enter the store as immutable events. No renderer or transport
adapter mutates state directly.

The domain event vocabulary is:

- `StatusChanged`
- `DirectoryChanged`
- `TabCreated`
- `TabClosed`
- `PaneFocused`
- `PaneExited`

Zellij host updates are normalized into these events before reduction. A pipe
message is decoded into the same vocabulary. The reducer validates identity on
every relevant event and validates generation and sequence on status events.
Invalid, closed, or stale targets produce a diagnostic and leave state
unchanged. A valid message for a target not yet observed is held in a bounded
queue and retried after host-state updates.

Wire-level details are specified in `docs/protocol.md`. The protocol envelope is
versioned independently of the package version.

### Identity and ordering

State uses stable Zellij tab and pane IDs. Display position is read at render
time, so reordering a tab cannot transfer another tab's name or status.

Status sequence numbers are monotonic within their documented source scope. The
store retains the greatest accepted value for that scope and rejects values less
than or equal to it. Generations identify logical agent runs. Once the store
observes a newer generation for a pane and agent, an event from an older
generation cannot change that record even if it has a larger sequence.

### Agent state

Agent state is semantic rather than color-based:

| State | Default badge |
| --- | --- |
| `Idle` | `💤` |
| `Running` | `🚀` |
| `Complete` | `✅` |
| `Error` | `❌` |

The latest accepted observation for every pane-and-agent record in a tab reduces
deterministically using the precedence in the constraints section. Generations
order each record; records do not need to share one generation number. Colors
may reinforce state but never replace the badge.

### Directory naming

The resolver chooses an automatic name using the first usable candidate:

1. focused pane working directory;
2. last focused pane working directory;
3. first terminal pane working directory;
4. tab working directory;
5. existing tab name;
6. `tab-<id>`.

Generated labels show the basename. When basenames collide, the resolver adds
the shortest parent suffix that makes every visible generated label unique.
Paths are handled as path data rather than assumed to be UTF-8; lossy display is
allowed, panics are not.

A name observed as user-assigned is stored separately from the latest generated
name and always wins. Restoring automatic naming clears the override rather than
guessing from the displayed text.

### Rendering

The renderer is a pure function of configuration, available columns, active tab,
ordered tab snapshots, and optional frame time. It does not query panes or modify
the store.

Layout preserves, in order, the tab number, semantic badge, and as much of the
label as fits. The active tab is highlighted through Zellij's theme when color
is enabled. Truncation occurs only on Unicode boundaries and uses display width
rather than byte length. Narrow layouts degrade by removing optional decoration
before meaningful state.

Theme values are resolved from configuration and Zellij context. Missing color
support produces a usable badge-only representation.

### Configuration

Plugin configuration is supplied as KDL and divided into stable conceptual
blocks:

- `theme`
- `badges`
- `layout`
- `behavior`
- `animation`
- `debug`

Parsing applies defaults first, validates known values, and treats malformed
optional settings as recoverable configuration errors. Exact keys and defaults
are documented in `docs/configuration.md` and may be extended additively.

### Error and logging policy

Errors are typed with context and handled at the nearest boundary that can make
a useful decision. Transport input errors are rejected; missing pane information
falls through the directory candidate list; rendering errors fall back to a
minimal safe representation.

Logging levels have consistent intent:

- `TRACE`: received and normalized events;
- `DEBUG`: accepted state transitions;
- `INFO`: startup and shutdown;
- `WARN`: recoverable invalid input or missing context;
- `ERROR`: unexpected failures that prevent an intended operation.

Logs must not include terminal contents, prompts, or environment secrets.

## Tradeoffs

- Named pipes provide simple event delivery but no durability. A plugin that was
  not listening cannot replay a status unless a future persistence adapter is
  enabled or a producer republishes it.
- Stable IDs keep reordered state correct but require explicit cleanup on close
  events and defensive handling when updates race with closure.
- A shared reducer avoids duplicated behavior but requires the shared crate to
  compile cleanly for both native and WASM targets.
- Strict sequence rejection makes reduction deterministic but requires each
  producer to maintain its source sequence correctly.
- Parent-path disambiguation improves clarity but can consume width; responsive
  truncation must run after labels are made unique.
- A small public protocol improves interoperability but creates a compatibility
  obligation. Internal Rust APIs intentionally remain narrower.

## Future Extension Points

The design permits additions without changing the core reducer:

- a persistent snapshot or append-only event-log adapter;
- an MCP or local JSON API that publishes the same protocol events;
- remote, Docker, or Kubernetes producers with distinct source identities;
- desktop notifications driven by accepted state transitions;
- timeline and task-graph views over recorded events;
- alternative renderers for compact, accessibility, or diagnostics modes; and
- protocol bridges for agents that cannot invoke `zja` directly.

These are extension points, not commitments. Agent orchestration, prompt
management, LLM integration, terminal emulation, Git operations, and cloud
backend behavior remain outside this project.

## Verification Strategy

Unit tests cover basename extraction, duplicate disambiguation, Unicode-safe
truncation, state aggregation, and stale rejection. Property tests generate
arbitrary create, close, move, rename, status, and focus sequences and assert
that reduction never panics and core invariants hold.

Integration tests exercise a complete tab lifecycle: create, automatic rename,
directory change, running status, successful and failed completion, manual-name
override, restoration of automatic naming, reorder, and closure.

Performance budgets are checked with representative event batches and render
snapshots; they are not enforced through sleeps in correctness tests.

## Compatibility Rules

1. Unknown protocol versions are rejected without changing state.
2. Unknown event kinds are rejected unless a later protocol explicitly defines
   an extension rule.
3. New optional fields may be added with defaults.
4. Existing field meanings and state precedence do not change within a protocol
   version.
5. CLI flags and configuration keys follow normal semantic-versioning rules.
6. Deprecated protocol variants remain decodable for a documented transition
   window before removal in a major release.
