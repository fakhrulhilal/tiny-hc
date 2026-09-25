#!/usr/bin/env bash
# Builds tiny-hc (HTTP + HTTPS via a trimmed mbedTLS) for one Rust target and
# copies it into an output dir.
#
#   scripts/build.sh <rust-target> [out-dir]
#
# Uses the `dist` profile with std rebuilt for size (nightly -Zbuild-std).
#
# Linux targets (musl, fully static) are built with cargo-zigbuild (needs
# `zig` and `cargo install cargo-zigbuild`).
set -euo pipefail

target="${1:?usage: scripts/build.sh <rust-target> [out-dir]}"
out="${2:-dist/$target}"

cargo=(cargo build)
[[ "$target" == *-linux-* ]] && cargo=(cargo zigbuild)
ext=""
[[ "$target" == *windows* ]] && ext=".exe"

"$(dirname "$0")/fetch-mbedtls.sh"

# shellcheck disable=SC2054 # the comma belongs to -Zbuild-std
flags=(--locked --profile dist --target "$target"
  -Zbuild-std=std,panic_abort -Zbuild-std-features=optimize_for_size)
"${cargo[@]}" "${flags[@]}" --bin tiny-hc

mkdir -p "$out"
cp "target/$target/dist/tiny-hc$ext" "$out/"
ls -l "$out"
