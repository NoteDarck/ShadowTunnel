#!/usr/bin/env bash
set -euo pipefail

: "${CARGO:=cargo}"
command -v "$CARGO" >/dev/null || { printf '%s\n' 'error: cargo is required (install Rust via rustup).' >&2; exit 1; }
command -v tar >/dev/null || { printf '%s\n' 'error: tar is required.' >&2; exit 1; }
"$CARGO" --version >/dev/null

root_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
target=x86_64-unknown-linux-gnu
output_dir="$root_dir/dist"
archive="$output_dir/shadowtunnel-agent-linux-x86_64.tar.gz"
mkdir -p "$output_dir"
"$CARGO" build --release -p shadowtunnel-agent --target "$target"
tar -czf "$archive" -C "$root_dir/target/$target/release" shadowtunnel-agent
printf 'Artifact: %s\n' "$archive"
