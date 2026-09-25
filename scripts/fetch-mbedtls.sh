#!/usr/bin/env bash
# Downloads the pinned mbedTLS release into vendor/mbedtls (compiled into
# tiny-hc by build.rs for HTTPS). Safe to re-run.
set -euo pipefail

version="4.2.0"
sha256="2bed9d713b4668f76553b097e72b8aa30bc8f112a940d7ae228d524bbde6ffea"

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/vendor/mbedtls"
if [[ -f "$dest/.version" && "$(cat "$dest/.version")" == "$version" ]]; then
  echo "mbedTLS $version already in $dest"
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
archive="$tmp/mbedtls.tar.bz2"
curl -fsSL -o "$archive" "https://github.com/Mbed-TLS/mbedtls/releases/download/mbedtls-$version/mbedtls-$version.tar.bz2"

if command -v sha256sum >/dev/null; then
  actual="$(sha256sum "$archive" | cut -d' ' -f1)"
else
  actual="$(shasum -a 256 "$archive" | cut -d' ' -f1)"
fi
if [[ "$actual" != "$sha256" ]]; then
  echo "checksum mismatch for mbedtls-$version: $actual" >&2
  exit 1
fi

tar -xjf "$archive" -C "$tmp"
rm -rf "$dest"
mkdir -p "$(dirname "$dest")"
mv "$tmp/mbedtls-$version" "$dest"
echo "$version" > "$dest/.version"
echo "mbedTLS $version extracted to $dest"
