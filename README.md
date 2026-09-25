# tiny-hc

[![Release](https://img.shields.io/github/v/release/fakhrulhilal/tiny-hc?sort=semver&logo=github&label=release)](https://github.com/fakhrulhilal/tiny-hc/releases/latest)
[![Docker Hub](https://img.shields.io/docker/v/iroel/tiny-hc?sort=semver&logo=docker&label=Docker%20Hub)](https://hub.docker.com/r/iroel/tiny-hc)
[![ghcr.io](https://img.shields.io/badge/ghcr.io-fakhrulhilal%2Ftiny--hc-blue?logo=github)](https://github.com/fakhrulhilal/tiny-hc/pkgs/container/tiny-hc)
[![Image size](https://img.shields.io/docker/image-size/iroel/tiny-hc?sort=semver&logo=docker&label=image%20size)](https://hub.docker.com/r/iroel/tiny-hc)
[![License](https://img.shields.io/github/license/fakhrulhilal/tiny-hc)](LICENSE)

A tiny HTTP/HTTPS health check for containers, inspired by
[microcheck](https://github.com/tarampampam/microcheck). It sends one `GET`
request and exits `0` when the response is healthy, `1` otherwise — exactly
what Docker's `HEALTHCHECK` expects. One small binary (about 300 KB) handles
both `http://` and `https://` and can follow redirects (for services that
redirect plain HTTP to HTTPS). It is static (Linux) with no runtime
dependencies, so it works in distroless/scratch images that have no shell and
no `curl`.

### HTTPS does not verify certificates

Like microcheck, `tiny-hc` **never verifies the server certificate**:
self-signed, expired, wrong-host and untrusted certificates are all accepted.
A health check normally talks to its own container, often over a self-signed
certificate, and skipping verification keeps the binary small (no root
store, no chain validation). The connection is still encrypted, but not
authenticated — do not use `tiny-hc` where that matters.

TLS 1.2 and 1.3 are supported, with AES-GCM, X25519/P-256/P-384 key exchange
and RSA (up to 4096 bits) or ECDSA server keys. That covers practically every
server today; very old servers (TLS 1.0/1.1, RC4, CBC-only or DHE-only) are
rejected.

## Usage

```text
tiny-hc [OPTIONS] <URL>

  -t, --timeout <SECONDS>         Whole check timeout, fractions allowed [default: 5]
      --basic-auth <USER:PASS>    Send HTTP basic authentication
      --expect-response <TEXT>    Also require the body to contain TEXT as whole word(s), any case
  -L, --location                  Follow redirects
      --max-redirs <NUM>          Maximum redirects to follow with -L [default: 3]
  -h, --help                      Print help
  -V, --version                   Print version
```

A check is **healthy** when the final response has a `2xx` status and, if
`--expect-response` is given, its body contains that text as whole word(s),
ignoring case: `healthy` matches a body of `Healthy`, but `health` does not;
`UP` matches `{"status":"UP"}` but not `UPGRADING`. The first 1 MiB of the body
is searched. On failure the reason is printed to stderr, which Docker shows in
`docker inspect`.

Redirects (301, 302, 303, 307, 308) are unhealthy unless `-L` is given, like
curl. With `-L` they are followed, up to `--max-redirs` (default 3), all within
the one `--timeout`; this is what services that redirect `http://` to
`https://` need.

```sh
tiny-hc http://127.0.0.1:8080/health
tiny-hc --timeout 2 --basic-auth admin:secret http://localhost/status
tiny-hc --expect-response healthy https://localhost:8443/actuator/health
tiny-hc -L http://example.com/health        # follows a redirect to https://
```

Credentials can also be put in the URL (`http://user:pass@host/`);
`--basic-auth` wins if both are set. When following redirects, credentials are
only sent to the host they were given for (an `http://` to `https://` redirect
on the same host keeps them), like curl.

## Install

Each [release](https://github.com/fakhrulhilal/tiny-hc/releases/latest) has
one archive per platform, containing the `tiny-hc` binary:

| Platform           | Asset                                      |
|--------------------|--------------------------------------------|
| Linux x86 64-bit   | `tiny-hc-x86_64-unknown-linux-musl.tar.gz` |
| Linux x86 32-bit   | `tiny-hc-i686-unknown-linux-musl.tar.gz`   |
| Windows x86 64-bit | `tiny-hc-x86_64-pc-windows-msvc.zip`       |
| Windows x86 32-bit | `tiny-hc-i686-pc-windows-msvc.zip`         |
| macOS arm64        | `tiny-hc-aarch64-apple-darwin.tar.gz`      |

The Linux binaries are statically linked against musl, so the same file runs
on Debian, Ubuntu, Alpine, distroless or scratch — no libc needed from the
host. The x86 32-bit builds need a CPU with SSE2 (Pentium 4 or newer).
Windows builds link the C runtime statically. `SHA256SUMS` lists the checksums
of all assets.

### Manual download

Linux (use `i686` for 32-bit):

```sh
curl -fsSL https://github.com/fakhrulhilal/tiny-hc/releases/latest/download/tiny-hc-x86_64-unknown-linux-musl.tar.gz \
  | sudo tar -xz -C /usr/local/bin tiny-hc
```

macOS (Apple silicon):

```sh
curl -fsSL https://github.com/fakhrulhilal/tiny-hc/releases/latest/download/tiny-hc-aarch64-apple-darwin.tar.gz \
  | sudo tar -xz -C /usr/local/bin tiny-hc
```

Windows (PowerShell; use `i686` for 32-bit):

```powershell
$zip = "$env:TEMP\tiny-hc.zip"
Invoke-WebRequest https://github.com/fakhrulhilal/tiny-hc/releases/latest/download/tiny-hc-x86_64-pc-windows-msvc.zip -OutFile $zip
Expand-Archive $zip -DestinationPath "$env:LOCALAPPDATA\tiny-hc" -Force
# then add %LOCALAPPDATA%\tiny-hc to PATH
```

### Verifying downloads

Every release archive, and every binary inside them, has a signed
[GitHub artifact attestation](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations)
(SLSA build provenance) proving it was built by this repository's release
workflow. mise checks it automatically on install. To check a file by hand
with the [GitHub CLI](https://cli.github.com):

```sh
gh attestation verify tiny-hc-x86_64-unknown-linux-musl.tar.gz --repo fakhrulhilal/tiny-hc
gh attestation verify /usr/local/bin/tiny-hc --repo fakhrulhilal/tiny-hc
```

### mise

```sh
mise use -g github:fakhrulhilal/tiny-hc
```

mise picks the right asset for the OS and CPU, verifies its GitHub artifact
attestation, and puts `tiny-hc` on the `PATH`. Or in `mise.toml`:

```toml
[tools]
"github:fakhrulhilal/tiny-hc" = "latest"
```

mise ignores releases younger than 24 hours by default (its
`minimum_release_age` setting), so right after a release it reports "no
versions found". To install a release published today:

```sh
mise use -g --minimum-release-age 0 github:fakhrulhilal/tiny-hc
```

### Docker

The image is published to
[Docker Hub](https://hub.docker.com/r/iroel/tiny-hc) (`iroel/tiny-hc`) and the
[GitHub Container Registry](https://github.com/fakhrulhilal/tiny-hc/pkgs/container/tiny-hc)
(`ghcr.io/fakhrulhilal/tiny-hc`), with the same tags on both, for:

| Platform    | Architecture                              |
|-------------|-------------------------------------------|
| linux/amd64 | x86 64-bit                                |
| linux/386   | x86 32-bit                                |
| linux/arm64 | ARM 64-bit (e.g. Docker on Apple silicon) |

The image is built `FROM scratch`: it contains `/bin/tiny-hc` and nothing
else — no shell. The usual way to use it is to copy the binary into your own
image — any Linux base works:

```dockerfile
# Any base works: debian, ubuntu, alpine, gcr.io/distroless/static, scratch, ...
FROM debian:trixie-slim
COPY --from=iroel/tiny-hc:latest /bin/tiny-hc /bin/tiny-hc
HEALTHCHECK --interval=10s --timeout=3s CMD ["/bin/tiny-hc", "http://127.0.0.1:8080/health"]
```

```dockerfile
# Following redirects (e.g. http:// -> https://) and checking the body
HEALTHCHECK CMD ["/bin/tiny-hc", "-L", "--expect-response", "healthy", "http://127.0.0.1:8080/health"]
```

Use the exec form (`CMD ["..."]`) of `HEALTHCHECK`: the shell form needs
`/bin/sh`, which distroless images do not have.

The image also runs on its own (entrypoint `/bin/tiny-hc`):

```sh
docker run --rm --network host iroel/tiny-hc http://127.0.0.1:8080/health
```

### Windows containers

There is no Windows image. Download the Windows x86 64-bit archive from the
[releases page](https://github.com/fakhrulhilal/tiny-hc/releases) into your
image instead; set `TINY_HC_VERSION` to the release you want (`0.1.3` below is
only an example):

```dockerfile
# escape=`
FROM mcr.microsoft.com/windows/nanoserver:ltsc2022
ARG TINY_HC_VERSION=0.1.3
USER ContainerAdministrator
ADD https://github.com/fakhrulhilal/tiny-hc/releases/download/v${TINY_HC_VERSION}/tiny-hc-x86_64-pc-windows-msvc.zip C:/tiny-hc.zip
RUN mkdir C:\tiny-hc && tar -xf C:\tiny-hc.zip -C C:\tiny-hc && del C:\tiny-hc.zip
USER ContainerUser
HEALTHCHECK CMD ["C:\\tiny-hc\\tiny-hc.exe", "-L", "http://127.0.0.1:8080/health"]
```

## Build

### How it is kept small

- The HTTP/1.1 client is hand written on top of Rust's `std`; no Rust crates
  are linked in (`cc` is only used at build time).
- HTTPS uses [mbedTLS](https://github.com/Mbed-TLS/mbedtls) 4, compiled with
  our own trimmed configuration (`csrc/tiny_hc_*_config.h`): TLS 1.2/1.3
  client only, the algorithms listed above, no certificate verification.
  `build.rs` compiles it with the [`cc`](https://crates.io/crates/cc) crate
  (no CMake); `csrc/tiny_hc_tls.c` is a small wrapper, network I/O stays in
  Rust.
- Release builds rebuild `std` itself for size with nightly Rust
  (`-Zbuild-std`, `optimize_for_size`) and use `panic = "immediate-abort"`.
  With the prebuilt `std`, `tiny-hc` would be about 3.5× bigger.

### Requirements

- [rustup](https://rustup.rs). The nightly toolchain is pinned in
  `rust-toolchain.toml` and installed automatically (`rustup toolchain install`
  if your rustup does not). If you use mise, `mise.toml` pins the same toolchain
  (a global mise `rust` setting would otherwise override `rust-toolchain.toml`)
  plus zig.
- A C compiler (clang/gcc, MSVC on Windows) and the mbedTLS sources, downloaded (pinned version, checksum verified) into `vendor/mbedtls`
  by `scripts/fetch-mbedtls.sh`. Set `MBEDTLS_DIR` to use another copy.

### Development builds

```sh
scripts/fetch-mbedtls.sh     # once
cargo build --release        # -> target/release/tiny-hc
cargo test
scripts/tls-test.sh target/release/tiny-hc   # TLS compatibility (needs openssl)
```

These use the prebuilt `std`, so the binaries are bigger than the released
ones, but they build quickly.

### Release (size-optimised) builds

`scripts/build.sh <rust-target> [out-dir]` builds `tiny-hc` for one target
the way releases are built (`--profile dist` plus `-Zbuild-std`) into
`dist/<rust-target>/`; it fetches mbedTLS if needed. Linux targets are built
with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild), which
works from macOS, Linux or Windows:

```sh
mise install                 # or install zig 0.16 any other way
cargo install --locked cargo-zigbuild

scripts/build.sh x86_64-unknown-linux-musl
scripts/build.sh i686-unknown-linux-musl
scripts/build.sh aarch64-unknown-linux-musl  # Docker image only
scripts/build.sh aarch64-apple-darwin        # on a Mac
```

Windows targets (`x86_64-pc-windows-msvc`, `i686-pc-windows-msvc`) are built
on Windows with the MSVC build tools, running the script from Git Bash.

### Docker images

The Dockerfile does not compile anything; it packages the binaries from
`dist/`, so build those first:

```sh
scripts/build.sh x86_64-unknown-linux-musl
scripts/build.sh i686-unknown-linux-musl
scripts/build.sh aarch64-unknown-linux-musl
# one platform, loaded into the local image store
docker buildx build --load --platform linux/arm64 -t tiny-hc .
docker run --rm tiny-hc --version
```

Building all three platforms in one go (as CI does) needs a
`docker-container` builder (`docker buildx create --use`) plus `--push`, or
the containerd image store.

## License

[MIT](LICENSE). The bundled mbedTLS is licensed under
Apache-2.0 OR GPL-2.0-or-later.
