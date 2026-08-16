# Event Protocol

This document specifies protocol version 1 of the CLI-to-plugin event format.
It is a compatibility boundary: implementations may change, but a version 1
producer and consumer must agree on the behavior described here.

## Transport

Events travel through Zellij's named-pipe facility using the pipe name:

```text
zja.events
```

The native CLI invokes the equivalent of:

```console
zellij pipe --name zja.events -- '<json-event>'
```

Each pipe payload contains one UTF-8 JSON object and is limited to 65,536 bytes.
There is no delimiter inside the payload, acknowledgement message, filesystem
queue, socket, daemon, or polling loop.

Zellij host updates such as tab creation and pane focus are normalized to the
same domain event vocabulary inside the plugin. Producers should use `zja`
instead of constructing pipe commands unless they are implementing an
integration that needs the stable JSON boundary.

## Version 1 Envelope

The envelope and event payload are flattened into one object. A running-status
event looks like:

```json
{
  "version": 1,
  "type": "status_changed",
  "pane_id": 42,
  "agent_id": "default",
  "status": "running",
  "generation": 1,
  "sequence": 1
}
```

### Common fields

| Field | JSON type | Required | Meaning |
| --- | --- | --- | --- |
| `version` | integer | producers: yes | Protocol version; exactly `1` for this document |
| `type` | string | yes | Snake-case event discriminator |

Unsupported versions and malformed envelopes are rejected without changing
stored state. The version 1 decoder treats an omitted `version` as `1` for
compatibility with early producers, but conforming producers always emit it.

## `status_changed`

`status_changed` is the external event emitted by the `zja` CLI.

| Field | JSON type | Required | Meaning |
| --- | --- | --- | --- |
| `tab_id` | integer, `null`, or omitted | no | Optional explicit Zellij tab target |
| `pane_id` | non-negative integer | yes | Stable Zellij terminal pane ID |
| `agent_id` | string | producers: yes | Producer-defined identity within the pane |
| `status` | string | yes | `idle`, `running`, `complete`, or `error` |
| `generation` | non-negative integer | yes | Logical agent-run generation |
| `sequence` | non-negative integer | yes | Monotonic order within one generation stream |

Rust consumers represent numeric IDs, `generation`, and `sequence` as `u64`.
Numeric values outside that range are malformed. The decoder uses `default` as
the agent ID when an early producer omits `agent_id`; conforming producers emit
the field explicitly.

An omitted or `null` `tab_id` lets the plugin resolve the target tab from the
stable pane ID. The canonical serializer omits this field when it is not set. An
explicit tab ID is useful to integrations that already have Zellij tab context.
Tab position is never used as identity.

### Status semantics

The four states are semantic and map to default badges:

| Wire value | Meaning | Default badge |
| --- | --- | --- |
| `idle` | No current work | `💤` |
| `running` | Work is in progress | `🚀` |
| `complete` | Work finished successfully | `✅` |
| `error` | Work failed | `❌` |

`zja run -- <command>` emits `running` with sequence `0` before launch,
`complete` with sequence `2` after exit code `0`, and `error` with sequence `2`
after any non-zero exit. It reserves sequence `1` for an optional child hook and
exports the generation and sequence through the child's environment.

## Ordering and Aggregation

Status ordering is deterministic and independent of delivery timing.

1. An agent record is identified by `(pane_id, agent_id)`.
2. Within its current generation, a `sequence` less than or equal to the
   greatest accepted sequence is stale and has no effect.
3. Once a newer generation is known for an agent, events from lower generations
   are stale even if they carry a larger sequence.
4. An accepted newer generation replaces the current observation for that agent
   record.
5. The latest observations from all agent records in a tab reduce using
   `running > error > complete > idle`.

The precedence intentionally keeps a tab visibly busy while any current agent
observation is running. It also prevents one success observation from hiding
another agent's current error. Generation numbers are compared only within the
same `(pane_id, agent_id)` record, so different agents do not need coordinated
generation counters.

### Producer requirements

A producer should:

- keep `agent_id` stable for one logical agent source;
- increment `generation` when starting a new logical run;
- increase `sequence` for every event in that generation;
- reuse the same pane identity reported by Zellij; and
- serialize one complete event per named-pipe payload.

The bundled CLI supplies these values. Custom integrations are responsible for
maintaining them across the events they emit.

## Domain Event Vocabulary

The shared state machine recognizes these snake-case discriminators:

| Type | Fields beyond `version` and `type` | Purpose |
| --- | --- | --- |
| `status_changed` | `tab_id?`, `pane_id`, `agent_id`, `status`, `generation`, `sequence` | Update agent status |
| `directory_changed` | `tab_id`, `pane_id?`, `directory` | Update a tab or pane working directory |
| `tab_created` | `tab_id`, `position`, `directory?`, `existing_name?`, `manual_name?` | Add stable tab state |
| `tab_closed` | `tab_id` | Remove tab state and associated panes |
| `tab_moved` | `tab_id`, `position` | Change presentation order without changing identity |
| `tab_renamed` | `tab_id`, `name` | Record a manual tab name |
| `automatic_naming_restored` | `tab_id` | Clear a manual-name override |
| `pane_focused` | `tab_id`, `pane_id`, `directory?`, `is_terminal` | Update focused and last-focused pane context |
| `pane_exited` | `tab_id`, `pane_id` | Remove pane-associated state |

Fields marked `?` may be omitted or `null`. IDs are `u64`, `directory` and name
fields are strings, and `position` is a zero-based non-negative integer within
the consumer's platform range. If `pane_id` is omitted from
`directory_changed`, the directory applies to the tab. `is_terminal` is a
boolean and defaults to `true` when omitted.

The plugin accepts only `status_changed` from the external pipe. The Zellij
adapter creates the other variants in process to normalize host updates before
reduction. Their serialized forms are documented because they are part of the
shared version 1 event type, but custom publishers cannot compete with Zellij
as the authority for tab and pane lifecycle.

## Rejection Behavior

The consumer rejects an event without changing valid state when it has:

- more than 65,536 payload bytes;
- invalid UTF-8 or invalid JSON;
- a missing or incorrectly typed required field;
- an unsupported `version`;
- an unknown event discriminator;
- an invalid status value;
- a stale sequence or generation; or
- a value outside the implementation's numeric range.

Recoverable rejection may produce a bounded diagnostic entry. It must not panic
the plugin, remove unrelated state, or block later valid events.

A well-formed event whose target cannot yet be resolved from host state is not
applied immediately. The plugin keeps it in a bounded 128-event pending queue
and retries after tab or pane updates. An explicit `tab_id` is sufficient once
that tab exists, even if the pane has not appeared yet. When the queue is full,
the oldest pending event is dropped. A target that never becomes valid therefore
has no visible effect.

## Compatibility

Protocol versioning is independent of the package version.

Within version 1:

- field meanings and status precedence do not change;
- new optional fields may be added with safe defaults;
- consumers ignore unknown additional object fields for additive forward
  compatibility;
- producers must not depend on an extension until the target supports it; and
- producers must not emit a new discriminator to a consumer that only supports
  the types documented here.

A breaking field, type, ordering, or semantic change requires a new protocol
version. Unsupported versions are rejected rather than interpreted as the
closest known shape.

## Security and Privacy

Pipe payloads are untrusted input. Consumers validate them before reduction and
must not execute content from JSON fields. `agent_id` is an identifier, not a
command.

Events contain status and Zellij identity metadata only. Integrations should not
place prompts, terminal contents, credentials, environment secrets, or agent
conversation text in any field.
