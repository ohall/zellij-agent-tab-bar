# opencode Integration

The `opencode/` directory holds a small opencode plugin, `zja-status`, that
shows an opencode agent's lifecycle in the Agent-Aware Zellij Tab Bar. It is a
consumer of `zja`'s protocol, not a component of the Zellij plugin build.

## Purpose

The tab bar only knows what is reported to it. `zja run -- cmd` reports the
lifecycle of a process you launch yourself, but opencode runs its own agent loop
inside a pane, so nothing publishes status for it. `zja-status` bridges that gap:
it watches opencode's session bus and publishes the same `zja.events` protocol
messages the tab bar already understands.

## Mapping

| opencode event | Condition | `zja` state | Badge |
| --- | --- | --- | --- |
| `session.status` | status `busy` or `retry` | `running` | `🚀` |
| `session.error` | any | `error` | `❌` |
| `session.idle` | previous run failed | `error` | `❌` |
| `session.idle` | otherwise | `complete` | `✅` |

The plugin tracks one logical run per `busy → idle` transition:

- Entering `busy`/`retry` starts a fresh Unix-nanosecond generation.
- Every published event carries a monotonically increasing sequence within that
  generation, so the message `running → complete` or `running → error` orders
  correctly in the reducer.
- Returning to `idle` publishes the terminal state and resets the generation.

## Boundary

`zja-status` reports state only. It never mutates prompts, messages, model
parameters, permissions, or session content. This matches the project's rule
that integrations signal lifecycle state but the tab bar never orchestrates an
agent.

It also degrades cleanly outside Zellij:

- Without `ZELLIJ_PANE_ID`, it silently returns without spawning `zja`.
- If `zja` is missing or the pipe fails, `zja` reports best-effort and the
  plugin ignores spawn errors; the agent is never blocked.

## Packaging

`opencode/package.json` publishes the plugin as a scoped npm package
(`@ohall/zja-status`). opencode loads a single TypeScript file directly with
Bun, so there is no build step and no runtime dependency: the only `@opencode-ai/plugin`
import is `type Plugin`, which is erased at runtime. `@opencode-ai/plugin` is
therefore a `devDependency` only.

## Error reporting

A failed run must surface as `error`, not be overwritten by the subsequent
`idle → complete`. `zja-status` remembers `lastRunFailed` set on
`session.error`, resets it when the next run starts, and lets the closing
`session.idle` report `error`. The tab bar's precedence
(`running > error > complete > idle`) keeps an active `running` from being
hidden by an older terminal state, and a `complete` after a prior error is still
correctly demoted while the error record persists.

## See also

- `opencode/README.md` — install and usage.
- `opencode/zja-status.ts` — full source.
- `docs/protocol.md` — the wire format `zja` publishes.
- `ARCHITECTURE.md` — transport and reducer boundaries.