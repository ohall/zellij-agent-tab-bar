#!/bin/sh
set -eu

workspace_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin_dir=${ZJA_BIN_DIR:-"$HOME/.local/bin"}
config_dir=${ZELLIJ_CONFIG_DIR:-"${XDG_CONFIG_HOME:-$HOME/.config}/zellij"}
plugin_dir=${ZJA_PLUGIN_DIR:-"$config_dir/plugins"}

"$workspace_root/scripts/build.sh"
mkdir -p "$bin_dir" "$plugin_dir"
cp "$workspace_root/target/release/zja" "$bin_dir/zja"
cp "$workspace_root/plugins/zellij-agent-tab-bar.wasm" \
  "$plugin_dir/zellij-agent-tab-bar.wasm"

printf 'Installed zja to %s\n' "$bin_dir/zja"
printf 'Installed plugin to %s\n' "$plugin_dir/zellij-agent-tab-bar.wasm"
printf '\nReplace the tab-bar alias in config.kdl with:\n'
printf '    tab-bar location="file:%s"\n' "$plugin_dir/zellij-agent-tab-bar.wasm"
printf '\nRestart Zellij after changing the alias.\n'
