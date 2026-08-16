# Agent-Aware Zellij Tab Bar

A small, event-driven Zellij tab bar that names tabs from their working
directories and shows agent lifecycle state.

```text
1 💤 api   2 🚀 auth   3 ✅ web   4 ❌ infra
```

| Badge | State |
| --- | --- |
| `💤` | Idle |
| `🚀` | Running |
| `✅` | Complete |
| `❌` | Error |

The project reports and visualizes state only. It does not orchestrate agents,
manage prompts, call LLMs, emulate a terminal, operate on Git repositories, or
require a cloud service.

## Features

- Event-driven status updates with no polling or daemon
- Automatic directory-based tab names
- Manual tab names that override generated names
- Duplicate-name disambiguation using the shortest useful parent path
- Stable tab and pane identity across reordering and closure
- Unicode-safe, width-aware, responsive rendering
- Theme-aware color with meaningful badge-only output
- Versioned JSON messages over a Zellij named pipe
- A wrapper command that reports process success or failure automatically

## Requirements

- Zellij
- Rust stable, only when building from source
- A shell that can place `zja` on `PATH`

The workspace supports Rust 1.92 and newer. The plugin builds for
`wasm32-wasip1`.

## Install From Source

From the repository root:

```console
./scripts/install.sh
```

The installer:

1. builds the native `zja` CLI and the WASM plugin;
2. installs `zja` in `${ZJA_BIN_DIR:-$HOME/.local/bin}`;
3. installs the plugin under the Zellij configuration directory; and
4. prints the complete `tab-bar` alias to place in `config.kdl`.

The plugin directory is selected from `ZJA_PLUGIN_DIR`, or from the `plugins`
directory below `ZELLIJ_CONFIG_DIR`, `XDG_CONFIG_HOME`, or
`$HOME/.config/zellij`. Make sure the CLI install directory is on `PATH`.

Replace the existing `tab-bar` entry in the `plugins` block of the Zellij
configuration with the alias printed by the installer. The shape is:

```kdl
plugins {
    tab-bar location="file:__ABSOLUTE_PLUGIN_PATH__"
}
```

Zellij requires a concrete plugin path; use the installer's rendered line rather
than the placeholder. Restart Zellij after changing a plugin alias.

On first load, allow the plugin's one-time `ReadApplicationState` permission
request. It needs tab, pane, focus, and working-directory metadata. It does not
request permission to run commands, write files, or change application state.

To choose other install roots:

```console
ZJA_BIN_DIR="$HOME/bin" ZELLIJ_CONFIG_DIR="$HOME/.config/zellij" \
  ./scripts/install.sh
```

## Usage

Run `zja` inside a Zellij pane. It uses the current Zellij pane as the status
target.

Report a state directly:

```console
zja status idle
zja status running
zja status complete
zja status error
```

Wrap an agent or any other process:

```console
zja run -- codex
zja run -- claude
zja run -- hermes
```

`zja run` reports `Running` before launching the child. Exit code `0` reports
`Complete`; a non-zero exit reports `Error`. The wrapper returns the child's exit
code.

Direct status commands are useful for shell hooks and integrations. Prefer
`zja run` when one command owns the complete lifecycle.

Live agents such as opencode run their own loop inside a pane, so they need a
bridge that publishes status on their behalf. See **opencode integration** below
and `docs/opencode.md`.

Run `zja --help` and `zja <command> --help` for the options supported by the
installed version. See `docs/cli.md` for explicit targeting, agent identities,
generation/sequence ordering, environment variables, and failure behavior.

## Configuration

Configuration values are children of the `tab-bar` plugin alias:

```kdl
plugins {
    tab-bar location="file:__ABSOLUTE_PLUGIN_PATH__" {
        badge_idle "💤"
        badge_running "🚀"
        badge_complete "✅"
        badge_error "❌"

        layout_separator "   "
        layout_max_tab_width 32
        layout_show_index true

        behavior_auto_name true
        theme_color true
        debug false
    }
}
```

All values are optional. Invalid values fall back to safe defaults rather than
crashing the plugin. See `docs/configuration.md` for the complete reference and
`examples/tab-bar.kdl` for a template.

## Naming

Automatic labels use the first available source:

1. focused pane working directory;
2. last focused pane working directory;
3. first terminal pane working directory;
4. tab working directory;
5. existing tab name; or
6. `tab-<id>`.

Only the basename is shown until duplicate basenames need parent context. A
manual Zellij tab rename always wins over an automatic label.

To restore automatic naming, enter Zellij's Rename Tab mode, delete the entire
name, and confirm. Zellij's canonical empty-name form (`Tab #N`) is also treated
as automatic.

## opencode integration

The `opencode/` directory contains `zja-status`, an opencode plugin that shows
an opencode agent's lifecycle in the tab bar. It listens to opencode session
events and publishes `running`, `complete`, or `error` through the same
`zja` protocol, so the tab shows `🚀` while the agent works and `✅`/`❌` when
it finishes.

Install it from npm in `opencode.json`:

```jsonc
{ "plugin": ["@ohall/zja-status"] }
```

It has no build step and is inert outside Zellij. See `docs/opencode.md` for the
event mapping and `opencode/README.md` for usage.

## How It Works

The CLI serializes a versioned event and sends it through the `zja.events`
Zellij named pipe. The plugin validates the message, reduces it with Zellij's
tab and pane events, resolves names, and renders an immutable snapshot. There is
no state-file poller, local socket, or background service.

See:

- `ARCHITECTURE.md` for decisions and boundaries
- `docs/cli.md` for the complete command reference
- `docs/protocol.md` for the CLI-to-plugin wire format
- `docs/configuration.md` for every plugin setting
- `docs/opencode.md` for the opencode integration
- `opencode/README.md` for installing the opencode plugin
- `CONTRIBUTING.md` for development and quality gates
- `SECURITY.md` for private vulnerability reporting

## Build and Test

```console
./scripts/build.sh
cargo test --workspace
```

Contributors with all quality-gate tools installed can run:

```console
./scripts/check.sh
```

## License

MIT. See `LICENSE`.
