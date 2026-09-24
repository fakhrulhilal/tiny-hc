#!/usr/bin/env bash
# Prints the image tags for one flavour, one per line.
#
#   scripts/docker-tags.sh <image> <flavour|""> <version>
#
# A release version (v1.2.3) expands to 1.2.3, 1.2, 1 and latest; anything
# else (pre-releases, "dev") only to itself. The flavour is appended as a
# suffix (1.2.3-linux), and replaces "latest" (linux).
set -euo pipefail

image="$1"
flavour="$2"
version="${3#v}"

aliases=("$version")
if [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.[0-9]+$ ]]; then
  aliases+=("${BASH_REMATCH[1]}.${BASH_REMATCH[2]}" "${BASH_REMATCH[1]}" latest)
fi

for alias in "${aliases[@]}"; do
  if [[ -z "$flavour" ]]; then
    echo "$image:$alias"
  elif [[ "$alias" == latest ]]; then
    echo "$image:$flavour"
  else
    echo "$image:$alias-$flavour"
  fi
done
