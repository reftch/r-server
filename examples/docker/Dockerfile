# Stage 1: Build with musl for static linking
# The musl target produces fully static binaries
FROM rust:1.96.1-alpine as builder

ARG ARTIFACT=rest
ARG TARGETARCH=aarch64
# ARG TARGETARCH=x86_64

# Install musl-dev for static compilation
# Also install build essentials needed by some crates
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY examples ./examples

# Build a statically linked binary
# The musl target produces a fully static binary by default
RUN cargo build --release --example $ARTIFACT --target $TARGETARCH-unknown-linux-musl && \
    cp target/$TARGETARCH-unknown-linux-musl/release/examples/$ARTIFACT /application

# Stage 2: Use scratch for the smallest possible image
# Scratch is an empty image - just the binary and nothing else
FROM scratch

# Copy SSL certificates for HTTPS support
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY cert.pem /cert.pem
COPY key.pem /key.pem

# Copy the statically linked binary
COPY --from=builder /application /application

# Scratch images have no shell, so we use the exec form
ENTRYPOINT ["/application"]