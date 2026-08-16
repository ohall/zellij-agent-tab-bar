# zja-status

An [opencode](https://opencode.ai) plugin that reports session activity to the
[Agent-Aware Zellij Tab Bar](..) by publishing `zja` status events on the
`zja.events` Zellij named pipe. While you use an agent, the tab holding it shows
a running badge; when it finishes, the tab shows success or failure.

| opencode activity | Badge shown |
| --- | --- |
| Session busy / retrying | `🚀` running |
| Session idle (success) | `✅` complete |
| Session error | `❌` error |

The plugin only acts inside Zellij: it waits for `ZELLIJ_PANE_ID` and deletes
itself privately when `zja` or the pipe is unavailable, so it is harmless
outside a Zellij session. It never changes prompts, conversation, or model
behavior.

## Install

Requires the `zja` CLI from the Agent-Aware Zellij Tab Bar on `PATH`, and
opencode running as a pane inside a Zellij session.

From npm:

```jsonc
{
  "plugin": ["@ohall/zja-status"]
}
```

From this repository (development):

```jsonc
{
  "plugin": ["file:./opencode/zja-status.ts"]
}
```

Restart opencode after changing `plugin`. The tab bar plugin must already be
installed as described in the repository README.

## How it works

`zja-status` subscribes to opencode's session bus and maps events to `zja`:

- `session.status` with status `busy` or `retry` reports `running`;
- `session.error` reports `error`;
- `session.idle` reports `complete`, or `error` when the preceding run failed.

Each start of a run begins a fresh generation, and events within that run carry
increasing sequence numbers so the tab bar's reducer orders them correctly. One
run therefore transitions `running → complete` or `running → error`.

## Build

The plugin is a single TypeScript file and has no build step. opencode runs it
directly. Install the type dependency for editing:

```console
cd opencode
npm install
```

See `docs/opencode.md` in the repository for the integration design and `docs/protocol.md`
for the wire format `zja` uses.

## License

MIT. See `LICENSE` in the repository root.