# Build stage
FROM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    gcc \
    openssl-dev \
    openssl-libs-static

WORKDIR /app

# Copy workspace Cargo files
COPY Cargo.toml Cargo.lock ./
COPY compiler/ compiler/
COPY sparkler/ sparkler/
COPY std/ std/
COPY bytecode-viewer/ bytecode-viewer/
COPY src/ src/

# Build in release mode
RUN cargo build --release

# Runtime stage
FROM alpine:3.23.3

RUN apk add --no-cache libgcc

WORKDIR /app

# Copy the built binary
COPY --from=builder /app/target/release/bengal /app/bengal

ENTRYPOINT ["/app/bengal"]
