#!/usr/bin/env bash
# Checks tiny-hc against local `openssl s_server` instances covering every
# certificate type, TLS version and key-exchange group it is meant to support.
#
#   scripts/tls-test.sh <path/to/tiny-hc>
set -euo pipefail

bin="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
work="$(mktemp -d)"
trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$work"' EXIT
cd "$work"

cert() { openssl req -x509 -newkey "$2" -keyout "$1.key" -out "$1.crt" -days 2 -nodes -subj /CN=localhost 2>/dev/null; }
cert rsa2048 rsa:2048
cert rsa4096 rsa:4096
openssl ecparam -name prime256v1 -out p256.pem && cert ec256 ec:p256.pem
openssl ecparam -name secp384r1 -out p384.pem && cert ec384 ec:p384.pem

port=19400 failed=0
check() { # <name> <cert> <s_server args...>
  local name=$1 crt=$2 pid out
  shift 2
  port=$((port + 1))
  openssl s_server -accept "$port" -cert "$crt.crt" -key "$crt.key" -www -quiet "$@" >/dev/null 2>&1 &
  pid=$!
  for _ in 1 2 3 4 5 6 7 8 9 10; do (echo > "/dev/tcp/127.0.0.1/$port") 2>/dev/null && break; sleep 0.2; done
  if out=$("$bin" -t 5 --expect-response s_server "https://127.0.0.1:$port/" 2>&1); then
    echo "ok    $name"
  else
    echo "FAIL  $name: $out"
    failed=1
  fi
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
}

for c in rsa2048 rsa4096 ec256 ec384; do
  check "TLS 1.3, $c" "$c" -tls1_3
  check "TLS 1.2, $c" "$c" -tls1_2
done
for g in X25519 P-256 P-384; do
  check "TLS 1.3, rsa2048, $g" rsa2048 -tls1_3 -groups "$g"
  check "TLS 1.2, ec256, $g" ec256 -tls1_2 -groups "$g"
done
check "TLS 1.3, TLS_AES_128_GCM_SHA256" rsa2048 -tls1_3 -ciphersuites TLS_AES_128_GCM_SHA256
check "TLS 1.3, TLS_AES_256_GCM_SHA384" rsa2048 -tls1_3 -ciphersuites TLS_AES_256_GCM_SHA384
exit "$failed"
