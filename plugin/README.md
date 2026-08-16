# Zellij Agent Tab Bar Plugin

The plugin accepts flat Zellij KDL configuration keys. Missing or malformed
values fall back to the defaults below.

| Key | Default | Meaning |
| --- | --- | --- |
| `badge_idle` | `💤` | Idle agent badge |
| `badge_running` | `🚀` | Running agent badge |
| `badge_complete` | `✅` | Completed agent badge |
| `badge_error` | `❌` | Failed agent badge |
| `layout_separator` | three spaces | Text between visible tabs |
| `layout_max_tab_width` | `32` | Maximum display columns per tab |
| `layout_show_index` | `true` | Prefix labels with their one-based position |
| `behavior_auto_name` | `true` | Replace default `Tab #N` labels with pane directories |
| `theme_color` | `true` | Use Zellij's active theme colors |
| `debug` | `false` | Emit trace/debug event-log entries to stderr |

The plugin subscribes only to Zellij state-change events and CLI pipe input. It
does not subscribe to timers or poll the host.

Lifecycle, warning, and error entries are emitted by default. Setting `debug`
also emits trace and state-change entries; the in-memory log remains bounded.

For compatibility with the shared configuration model, dotted aliases such as
`badges.running`, `layout.separator`, `layout.max_name_width`,
`layout.show_index`, `behavior.automatic_naming`, `theme.use_color`, and
`debug.enabled` are also accepted. The flat keys above take precedence.
