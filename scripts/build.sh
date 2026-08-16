#!/bin/sh
set -eu

workspace_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target="wasm32-wasip1"

cd "$workspace_root"
if command -v rustup >/dev/null 2>&1; then
  rustup target add "$target"
fi
cargo build --release --target "$target" --package zellij-agent-tab-bar \
  --bin zellij-agent-tab-bar
"$workspace_root/scripts/verify-plugin.sh" \
  "target/$target/release/zellij-agent-tab-bar.wasm"
mkdir -p plugins
cp "target/$target/release/zellij-agent-tab-bar.wasm" \
  "plugins/zellij-agent-tab-bar.wasm"
cargo build --release --package zja

printf 'Built %s\n' "$workspace_root/plugins/zellij-agent-tab-bar.wasm"
printf 'Built %s\n' "$workspace_root/target/release/zja"
