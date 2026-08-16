# CLI Reference

`zja` reports status to Agent-Aware Zellij Tab Bar or wraps a child process and
reports its lifecycle automatically.

## Commands

```text
zja [GLOBAL OPTIONS] status <STATE> [STATUS OPTIONS]
zja [GLOBAL OPTIONS] run [RUN OPTIONS] -- <COMMAND> [ARGS...]
```

Global options precede the subcommand. Use `zja --help`,
`zja status --help`, or `zja run --help` for generated help.

## Global Options

| Option | Environment | Default | Purpose |
| --- | --- | --- | --- |
| `--pane-id <ID>` | `ZELLIJ_PANE_ID` | current Zellij pane | Target terminal pane |
| `--tab-id <ID>` | `ZJA_TAB_ID` | resolve from pane | Explicit stable tab target |
| `--zellij-bin <PATH>` | `ZJA_ZELLIJ_BIN` | `zellij` | Zellij executable to invoke |

Pane IDs may be unsigned integers or Zellij forms such as `terminal_42` and
`pane_42`. Plugin pane IDs are not valid status targets. Tab IDs are unsigned
integers, not one-based display positions.

Most users should rely on `ZELLIJ_PANE_ID` and leave every global option unset.
Explicit targeting is intended for integrations that already track stable
Zellij IDs. The binary override primarily supports custom installations and
tests.

## `status`

Report one of four states:

```console
zja status idle
zja status running
zja status complete
zja status error
```

Options:

| Option | Environment | Default | Purpose |
| --- | --- | --- | --- |
| `--agent-id <ID>` | `ZJA_AGENT_ID` | `default` | Identify one agent source in the pane |
| `--generation <N>` | `ZJA_GENERATION` | current Unix time in nanoseconds | Identify a logical run |
| `--sequence <N>` | `ZJA_SEQUENCE` | `0` | Order updates within the generation |

Example with an explicit target and ordering metadata:

```console
zja --pane-id terminal_42 --tab-id 7 \
  status running --agent-id reviewer --generation 12 --sequence 0
```

The pane is required. Outside Zellij, pass `--pane-id` or set
`ZELLIJ_PANE_ID`; otherwise `status` exits with an error.

When several direct commands describe one run, reuse the generation and
increase the sequence:

```console
export ZJA_GENERATION=12
export ZJA_AGENT_ID=reviewer
zja status running --sequence 0
# ...work...
zja status complete --sequence 1
```

An event whose generation or sequence is stale has no visible effect. See
`protocol.md` for exact ordering and aggregate precedence.

## `run`

Wrap a command after the required `--` separator:

```console
zja run -- codex
zja run -- claude --help
zja run --agent-id planner -- hermes task.md
```

`--agent-id <ID>` may also be supplied through `ZJA_AGENT_ID`. When it is
omitted, `run` uses the launched executable's basename, such as `codex`.

For each invocation, `run`:

1. creates a generation from the current Unix time in nanoseconds;
2. publishes `running` with sequence `0`;
3. starts the child with `ZJA_AGENT_ID`, `ZJA_GENERATION`, and
   `ZJA_SEQUENCE=1` in its environment;
4. publishes `complete` with sequence `2` after exit code `0`, or `error` with
   sequence `2` after a non-zero exit; and
5. returns the child's ordinary numeric exit code.

The reserved child sequence `1` allows one intermediate hook in the wrapped
process to publish a status in the same generation. A more elaborate producer
should manage its own increasing sequences through the protocol API.

Lifecycle reporting is best effort so transport failure never prevents the
requested command from running. When invoked outside Zellij without a pane ID,
`run` prints a warning, runs the child, and returns its result without reporting
status. A command-launch failure exits with an error.

## Transport Behavior

For each event, `zja` serializes versioned JSON and runs:

```console
zellij pipe --name zja.events -- '<json-event>'
```

A direct `status` command fails when serialization, process launch, or the
Zellij pipe command fails. `run` reports the same failures as warnings because
the child lifecycle remains its primary responsibility.

Do not pass secrets, prompts, or terminal contents through agent IDs. Protocol
events are intended to contain identity and lifecycle metadata only.
