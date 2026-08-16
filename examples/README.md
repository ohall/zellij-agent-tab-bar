# Examples

## `tab-bar.kdl`

Copy the settings into the `plugins` block of Zellij's `config.kdl`. Replace the
placeholder with the concrete plugin path printed by `scripts/install.sh`. Do
not keep two `tab-bar` aliases.

## `run-agent.sh`

This POSIX shell wrapper forwards an arbitrary command to `zja run`:

```console
./examples/run-agent.sh codex
./examples/run-agent.sh claude --help
```

It is intentionally thin; `zja` owns lifecycle reporting and exit-code
propagation.
