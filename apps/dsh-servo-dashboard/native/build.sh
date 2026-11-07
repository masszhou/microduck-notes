#!/bin/sh
set -eu
crate=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$crate/.." && pwd)
cargo build --offline --locked --release --manifest-path "$crate/Cargo.toml" --target-dir "$crate/target"
mkdir -p "$root/bin"
bin_dir=$(CDPATH= cd -P -- "$root/bin" && pwd)
# Verify resolved destinations before replacing the executable.
[ "$bin_dir" = "$root/bin" ] || { echo "refuse unexpected bin directory" >&2; exit 1; }
target="$bin_dir/servo-worker"
[ ! -L "$target" ] || { echo "refuse symlink binary target" >&2; exit 1; }
tmp=$(mktemp "$bin_dir/.servo-worker.XXXXXX")
case "${tmp:?}" in "$bin_dir"/.servo-worker.*) ;; *) echo "refuse unexpected temporary path" >&2; exit 1 ;; esac
[ -f "$tmp" ] && [ ! -L "$tmp" ]
cp "$crate/target/release/servo-worker" "$tmp"
chmod 755 "$tmp"
# A new inode avoids stale macOS executable code-signing cache entries.
mv -f "$tmp" "$target"
