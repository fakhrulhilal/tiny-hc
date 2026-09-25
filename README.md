# tiny-hc

A tiny HTTP/HTTPS health check for containers, inspired by
[microcheck](https://github.com/tarampampam/microcheck). It sends one `GET`
request and exits `0` when the response is healthy, `1` otherwise — exactly
what Docker's `HEALTHCHECK` expects. The binaries are static (Linux) and have no
runtime dependencies, so they work in distroless/scratch images that have no
shell and no `curl`.

## Two binaries: `tiny-hc` and `tiny-hc-tls`

HTTPS adds about 180 KB, more than the 100 KB threshold, so it is a build
flag (`--features tls`) and every release ships two binaries. Pick the one you
need:

| Binary        | Schemes          | Linux x86_64 | Linux x86 (i686) | macOS arm64 |
|---------------|------------------|-------------:|-----------------:|------------:|
| `tiny-hc`     | `http`           |       118 KB |           113 KB |      103 KB |
| `tiny-hc-tls` | `http` + `https` |       298 KB |           297 KB |      264 KB |

For comparison, on Linux x86_64 (static, stripped): microcheck's `httpcheck`
is 71 KB and its `httpscheck` 485 KB; curl 8.22 stripped down to HTTP only is
462 KB. Sizes vary a little between versions; each CI build prints the exact
sizes in its job summary and warns if HTTPS ever costs 100 KB or less (the
split would then not be worth it).

### HTTPS does not verify certificates

Like microcheck, `tiny-hc-tls` **never verifies the server certificate**:
self-signed, expired, wrong-host and untrusted certificates are all accepted.
A health check normally talks to its own container, often over a self-signed
certificate, and skipping verification keeps the binary small (no root
store, no chain validation). The connection is still encrypted, but not
authenticated — do not use `tiny-hc-tls` where that matters.

TLS 1.2 and 1.3 are supported, with AES-GCM, X25519/P-256/P-384 key exchange
and RSA (up to 4096 bits) or ECDSA server keys. That covers practically every
server today; very old servers (TLS 1.0/1.1, RC4, CBC-only or DHE-only) are
rejected.

## Usage

```text
tiny-hc [OPTIONS] <URL>

  -t, --timeout <SECONDS>         Whole request timeout, fractions allowed [default: 5]
      --basic-auth <USER:PASS>    Send HTTP basic authentication
      --expect-response <TEXT>    Also require the response body to contain TEXT
  -h, --help                      Print help
  -V, --version                   Print version
```

A check is **healthy** when the status code is `2xx` and, if
`--expect-response` is given, the response body contains that text (plain
substring, case sensitive; the first 1 MiB of the body is searched). Redirects
are not followed. On failure the reason is printed to stderr, which Docker
shows in `docker inspect`.

```sh
tiny-hc http://127.0.0.1:8080/health
tiny-hc --timeout 2 --basic-auth admin:secret http://localhost/status
tiny-hc-tls --expect-response 'healthy' https://localhost:8443/actuator/health
```

Credentials can also be put in the URL (`http://user:pass@host/`);
`--basic-auth` wins if both are set. `tiny-hc` exits `1` with a hint when it
is given an `https://` URL.

## Install

Release assets are named after the Rust target and contain both binaries:

| Platform       | Asset                                      |
|----------------|--------------------------------------------|
| Linux x86_64   | `tiny-hc-x86_64-unknown-linux-musl.tar.gz` |
| Linux x86      | `tiny-hc-i686-unknown-linux-musl.tar.gz`   |
| Windows x86_64 | `tiny-hc-x86_64-pc-windows-msvc.zip`       |
| Windows x86    | `tiny-hc-i686-pc-windows-msvc.zip`         |
| macOS arm64    | `tiny-hc-aarch64-apple-darwin.tar.gz`      |

The Linux binaries are statically linked against musl, so the same file runs
on Debian, Ubuntu, Alpine, distroless or scratch — no libc needed from the
host. The x86 (i686) builds need a CPU with SSE2 (Pentium 4 or newer).
Windows builds link the C runtime statically. `SHA256SUMS` lists the checksums
of all assets.

### Verifying downloads

Every release archive, and every binary inside them, has a signed
[GitHub artifact attestation](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations)
(SLSA build provenance) proving it was built by this repository's release
workflow. mise checks it automatically on install. To check a file by hand
with the [GitHub CLI](https://cli.github.com):

```sh
gh attestation verify tiny-hc-x86_64-unknown-linux-musl.tar.gz --repo fakhrulhilal/tiny-hc
gh attestation verify /usr/local/bin/tiny-hc-tls --repo fakhrulhilal/tiny-hc
```

### mise

```sh
mise use -g github:fakhrulhilal/tiny-hc
```

mise picks the right asset for the OS and CPU and puts both `tiny-hc` and
`tiny-hc-tls` on the `PATH`. Or in `mise.toml`:

```toml
[tools]
"github:fakhrulhilal/tiny-hc" = "latest"
```

### Docker

The image is published to the GitHub Container Registry and Docker Hub, for
**linux/amd64** (x86 64-bit) and **linux/386** (x86 32-bit):

| Registry   | Image                          | Tags                                |
|------------|--------------------------------|-------------------------------------|
| ghcr.io    | `ghcr.io/fakhrulhilal/tiny-hc` | `latest`, `1`, `1.2`, `1.2.3`       |
| Docker Hub | `iroel/tiny-hc`                | `latest`, `1`, `1.2`, `1.2.3`       |

(`1.2.3` stands for any released version; each GitHub release lists its exact
tags.) The image is built `FROM scratch`: it contains the two binaries in
`/bin` and nothing else — no shell. The usual way to use it is to copy the
binary into your own image — any Linux base works:

```dockerfile
# Any base works: debian, ubuntu, alpine, gcr.io/distroless/static, scratch, ...
FROM debian:trixie-slim
COPY --from=ghcr.io/fakhrulhilal/tiny-hc:1 /bin/tiny-hc /bin/tiny-hc
HEALTHCHECK --interval=10s --timeout=3s CMD ["/bin/tiny-hc", "http://127.0.0.1:8080/health"]
```

```dockerfile
# HTTPS (certificate not verified), checking the body
COPY --from=ghcr.io/fakhrulhilal/tiny-hc:1 /bin/tiny-hc-tls /bin/tiny-hc-tls
HEALTHCHECK CMD ["/bin/tiny-hc-tls", "--expect-response", "healthy", "https://127.0.0.1:8443/health"]
```

Use the exec form (`CMD ["..."]`) of `HEALTHCHECK`: the shell form needs
`/bin/sh`, which distroless images do not have.

The images also run on their own (entrypoint `/bin/tiny-hc`):

```sh
docker run --rm --network host ghcr.io/fakhrulhilal/tiny-hc http://127.0.0.1:8080/health
docker run --rm --entrypoint /bin/tiny-hc-tls ghcr.io/fakhrulhilal/tiny-hc https://example.com
```

There are no Windows images; on Windows use the release packages.

### Manual download

Linux (use `i686` for 32-bit):

```sh
curl -fsSL https://github.com/fakhrulhilal/tiny-hc/releases/latest/download/tiny-hc-x86_64-unknown-linux-musl.tar.gz \
  | sudo tar -xz -C /usr/local/bin tiny-hc tiny-hc-tls
```

macOS (Apple silicon):

```sh
curl -fsSL https://github.com/fakhrulhilal/tiny-hc/releases/latest/download/tiny-hc-aarch64-apple-darwin.tar.gz \
  | sudo tar -xz -C /usr/local/bin tiny-hc tiny-hc-tls
```

Windows (PowerShell; use `i686` for 32-bit):

```powershell
$zip = "$env:TEMP\tiny-hc.zip"
Invoke-WebRequest https://github.com/fakhrulhilal/tiny-hc/releases/latest/download/tiny-hc-x86_64-pc-windows-msvc.zip -OutFile $zip
Expand-Archive $zip -DestinationPath "$env:LOCALAPPDATA\tiny-hc" -Force
# then add %LOCALAPPDATA%\tiny-hc to PATH
```

Replace `latest/download` with `download/v1.2.3` to pin a version.

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
- For `tiny-hc-tls`: a C compiler (clang/gcc, MSVC on Windows) and the mbedTLS
  sources, downloaded (pinned version, checksum verified) into `vendor/mbedtls`
  by `scripts/fetch-mbedtls.sh`. Set `MBEDTLS_DIR` to use another copy.

### Development builds

```sh
scripts/fetch-mbedtls.sh     # once, for the tls feature

cargo build --release --bin tiny-hc                     # HTTP only
cargo build --release --bin tiny-hc-tls --features tls  # HTTP + HTTPS

cargo test                   # HTTP-only build
cargo test --features tls    # TLS build
```

These use the prebuilt `std`, so the binaries are bigger than the released
ones, but they build quickly.

### Release (size-optimised) builds

`scripts/build.sh <rust-target> [out-dir]` builds both binaries for one target
the way releases are built (`--profile dist` plus `-Zbuild-std`) into
`dist/<rust-target>/`; it fetches mbedTLS if needed. Linux targets are built
with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild), which
works from macOS, Linux or Windows:

```sh
mise install                 # or install zig 0.16 any other way
cargo install --locked cargo-zigbuild

scripts/build.sh x86_64-unknown-linux-musl
scripts/build.sh i686-unknown-linux-musl
scripts/build.sh aarch64-apple-darwin      # on a Mac
```

Windows targets (`x86_64-pc-windows-msvc`, `i686-pc-windows-msvc`) are built
on Windows with the MSVC build tools, running the script from Git Bash.

### Docker images

The Dockerfile does not compile anything; it packages the binaries from
`dist/`, so build those first:

```sh
scripts/build.sh x86_64-unknown-linux-musl
scripts/build.sh i686-unknown-linux-musl
# one platform, loaded into the local image store
docker buildx build --load --platform linux/amd64 -t tiny-hc .
docker buildx build --load --platform linux/386 -t tiny-hc:386 .
docker run --rm tiny-hc --version
```

Building both platforms in one go (as CI does) needs a `docker-container`
builder (`docker buildx create --use`) plus `--push`, or the containerd image
store.

## CI / releases

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `ci.yml` | push to `main`, pull requests | fmt, clippy, tests on Linux/macOS/Windows, builds every target, builds and tests the image (no push) |
| `release.yml` | tag `v*` | the builds above, signed build-provenance attestations for every archive and binary (verified before publishing), a check that all five packages (x86 64/32-bit Linux and Windows, macOS arm64) are present, a GitHub release with the archives and `SHA256SUMS`, the image pushed to `ghcr.io` and Docker Hub (every tag verified to contain linux/amd64 and linux/386), and the image tags added to the release notes |
| `build.yml`, `docker.yml` | reusable | the build matrix and the image jobs shared by the two above |

Images always go to `ghcr.io/<owner>/<repo>` (using the built-in
`GITHUB_TOKEN`). Publishing to Docker Hub as well is switched on in the
repository settings (*Settings → Secrets and variables → Actions*):

| Kind     | Name                 | Value |
|----------|----------------------|-------|
| Variable | `DOCKERHUB_USERNAME` | Docker Hub user that pushes; setting it enables Docker Hub |
| Secret   | `DOCKERHUB_TOKEN`    | a Docker Hub access token with *Read & Write* scope |
| Variable | `DOCKERHUB_IMAGE`    | optional, e.g. `myorg/tiny-hc`; defaults to `<DOCKERHUB_USERNAME>/tiny-hc` |

If the username is set but the token is missing, the release fails before
pushing anything.

To release: bump `version` in `Cargo.toml`, then `git tag v1.2.3 && git push --tags`.
Tags with a `-` (e.g. `v1.2.3-rc.1`) become pre-releases and do not move
`latest`, `1` or `1.2`.

To update mbedTLS: change `version` and `sha256` in `scripts/fetch-mbedtls.sh`
(the checksum is published next to each mbedTLS release).

## Exit codes

| Code | Meaning |
|------|---------|
| `0`  | healthy (or `--help` / `--version`) |
| `1`  | unhealthy, unreachable, timed out, or invalid arguments |

## License

The bundled mbedTLS is licensed under Apache-2.0 OR GPL-2.0-or-later.
