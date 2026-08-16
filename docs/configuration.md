# Configuration Reference

Agent-Aware Zellij Tab Bar reads configuration from its plugin alias in Zellij's
KDL configuration. Settings are optional and grouped by stable prefixes rather
than nested KDL blocks because Zellij passes plugin alias properties as a flat
map.

## Alias

The installer prints a complete alias containing the actual plugin path. Replace
Zellij's existing `tab-bar` alias with that line, then add settings as children:

```kdl
plugins {
    tab-bar location="file:__ABSOLUTE_PLUGIN_PATH__" {
        badge_running "▶"
        layout_max_tab_width 28
        theme_color true
    }
}
```

`__ABSOLUTE_PLUGIN_PATH__` is documentation syntax, not a literal usable path.
Use the value printed by `scripts/install.sh`.

Changes to a plugin alias require a Zellij restart.

## Settings

| Concept | Key | Type | Default | Effect |
| --- | --- | --- | --- | --- |
| Badges | `badge_idle` | string | `💤` | Badge for idle state |
| Badges | `badge_running` | string | `🚀` | Badge for running state |
| Badges | `badge_complete` | string | `✅` | Badge for successful completion |
| Badges | `badge_error` | string | `❌` | Badge for failed completion |
| Layout | `layout_separator` | string | three spaces | Text between rendered tabs |
| Layout | `layout_max_tab_width` | integer, 8–256 | `32` | Maximum display columns for one tab |
| Layout | `layout_show_index` | boolean | `true` | Show Zellij's tab index |
| Behavior | `behavior_auto_name` | boolean | `true` | Derive labels from pane directories |
| Theme | `theme_color` | boolean | `true` | Use theme-aware active/inactive tab colors |
| Debug | `debug` | boolean | `false` | Enable additional diagnostic logging |

Boolean values accept `true`, `1`, `yes`, or `on`, and `false`, `0`, `no`, or
`off`, without regard to case. Width is measured in terminal display columns,
not bytes or Unicode scalar values.

Unknown or malformed settings do not stop the plugin. The corresponding known
setting retains its default. This tolerance is for forward compatibility, not a
substitute for validating configuration before sharing it.

## Badges

Badges carry meaning even when color is unavailable. The defaults are Emoji
(`💤` idle, `🚀` running, `✅` complete, `❌` error) chosen to be instantly
readable. They render at two display columns each, so they take a little more
room than a single-column marker. They may contain any Unicode and more than
one display column, but wider badges leave less room for directory labels. An
empty badge value is invalid and restores that badge's default.

For ASCII-only environments:

```kdl
badge_idle "-"
badge_running "*"
badge_complete "+"
badge_error "!"
```

Keep badges distinct when `theme_color` is `false`.

## Layout

`layout_max_tab_width` caps each rendered tab before the full bar adapts to the
available width. The renderer truncates on Unicode boundaries and preserves
state indicators before optional label text.

Use an empty `layout_separator` for the most compact presentation, or a visible
delimiter when color is disabled:

```kdl
layout_separator " | "
layout_show_index false
```

## Automatic Naming

With `behavior_auto_name true`, the plugin resolves a generated name in this
order:

1. focused pane working directory;
2. last focused pane working directory;
3. first terminal pane working directory;
4. tab working directory;
5. existing tab name;
6. `tab-<id>`.

Generated names use a basename and add only enough parent path to distinguish
duplicates. A manual Zellij rename overrides the generated name.

To remove a manual override, enter Zellij's Rename Tab mode, clear all text, and
confirm. The empty name or any Zellij default `Tab #N` name restores automatic
resolution (Zellij keeps default names when tabs are closed and re-created, so
the number does not have to match the display position).

Set `behavior_auto_name false` to preserve Zellij's names without directory
resolution. Status badges still render.

## Theme

With `theme_color true`, the renderer uses the active Zellij palette and focus
state. With it disabled, output remains usable through the configured badges and
text alone; the active tab keeps a reverse-video cue instead of colors.

Status colors are drawn from the active theme's palette: running uses the
theme yellow, complete the theme green, and error the theme red. Idle badges are
dimmed rather than colored (so the Emoji defaults `💤🚀✅❌` are the distinction
when they render in their own colors). A badge whose status color matches its
tab background falls back to the tab's base foreground so it stays visible. Tab
indices are dimmed and tab names are emboldened on the active tab, so the badge
remains the strongest signal in each tab.

The configuration surface intentionally does not expose fixed RGB or
named-color values. This keeps output consistent with the active Zellij theme.

## Animation

Animation is reserved as a future configuration area. The initial plugin has no
animation timer: updates occur only in response to CLI or Zellij events. This
preserves the zero-polling architecture and avoids idle redraws.

## Debugging

Set `debug true` only while diagnosing plugin behavior:

```kdl
debug true
```

The plugin retains at most 128 diagnostic entries in memory. Lifecycle,
warning, and error entries are emitted even when debug mode is off; `debug true`
also emits lower-level event metadata and state transitions. Diagnostics must
not include pane contents, prompts, or environment secrets. Return the option
to `false` after collecting diagnostics to reduce noise.

## Complete Example

See `examples/tab-bar.kdl` for a configuration template with every supported
setting.
