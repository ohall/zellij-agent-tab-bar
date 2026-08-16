#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  printf 'Usage: %s <plugin.wasm>\n' "$0" >&2
  exit 2
fi

plugin_path=$1
if [ ! -f "$plugin_path" ]; then
  printf 'Plugin artifact not found: %s\n' "$plugin_path" >&2
  exit 1
fi

if command -v llvm-readobj >/dev/null 2>&1; then
  llvm_readobj=$(command -v llvm-readobj)
else
  rust_host=$(rustc -vV | sed -n 's/^host: //p')
  llvm_readobj="$(rustc --print sysroot)/lib/rustlib/$rust_host/bin/llvm-readobj"
fi

if [ ! -x "$llvm_readobj" ]; then
  printf '%s\n' 'llvm-readobj is required; install the llvm-tools-preview Rust component' >&2
  exit 1
fi

symbols=$("$llvm_readobj" --symbols "$plugin_path")
for export_name in _start load update pipe render plugin_version; do
  if ! printf '%s\n' "$symbols" | grep -Eq "^[[:space:]]*Name: ${export_name}$"; then
    printf 'Missing required WASM export: %s\n' "$export_name" >&2
    exit 1
  fi
done

printf 'Verified Zellij plugin exports in %s\n' "$plugin_path"
