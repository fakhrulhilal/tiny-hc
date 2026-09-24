# syntax=docker/dockerfile:1
#
# Linux image (linux/amd64, linux/386), assembled from the static musl
# binaries pre-built into dist/<rust-target>/ by scripts/build.sh. Built
# FROM scratch: no shell, no package manager, only the two binaries in /bin.
# The binaries need no libc from the host, so they can be copied into
# Debian, Ubuntu, Alpine, distroless or scratch images alike.
#
#   docker buildx build --load --platform linux/amd64 -t tiny-hc .
ARG TARGETARCH

FROM scratch AS bin-amd64
COPY dist/x86_64-unknown-linux-musl/tiny-hc dist/x86_64-unknown-linux-musl/tiny-hc-tls /
FROM scratch AS bin-386
COPY dist/i686-unknown-linux-musl/tiny-hc dist/i686-unknown-linux-musl/tiny-hc-tls /

FROM bin-${TARGETARCH} AS bin

FROM scratch
COPY --from=bin --chmod=0755 / /bin/
USER 65532:65532
ENTRYPOINT ["/bin/tiny-hc"]
