#!/usr/bin/env bash
# Prints the image tags for a version, one per line.
#
#   scripts/docker-tags.sh <image> <version>
#
# A release version (v1.2.3) expands to 1.2.3, 1.2, 1 and latest; anything
# else (pre-releases, "dev") only to itself.
set -euo pipefail

image="$1"
version="${2#v}"

echo "$image:$version"
if [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.[0-9]+$ ]]; then
  echo "$image:${BASH_REMATCH[1]}.${BASH_REMATCH[2]}"
  echo "$image:${BASH_REMATCH[1]}"
  echo "$image:latest"
fi
