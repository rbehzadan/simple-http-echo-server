# Use Alpine-based Rust image for musl builds (latest version for edition2024 support)
FROM rust:1.87-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev

# Set the working directory
WORKDIR /app

# Add the musl target for the current architecture
RUN case "$(uname -m)" in \
    x86_64) ARCH=x86_64 ;; \
    aarch64) ARCH=aarch64 ;; \
    *) echo "Unsupported architecture" && exit 1 ;; \
    esac && \
    rustup target add ${ARCH}-unknown-linux-musl

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./

# Copy the source code
COPY src ./src

# Build the application statically linked with musl
RUN case "$(uname -m)" in \
    x86_64) TARGET=x86_64-unknown-linux-musl ;; \
    aarch64) TARGET=aarch64-unknown-linux-musl ;; \
    esac && \
    cargo build --release --target ${TARGET} && \
    cp target/${TARGET}/release/simple-http-echo-server /app/simple-http-echo-server

# Use minimal Alpine runtime image
FROM alpine:3.19

# Install ca-certificates for HTTPS requests
RUN apk --no-cache add ca-certificates

# Create a non-root user
RUN adduser -D -s /bin/sh appuser

# Copy the statically linked binary from the builder stage
COPY --from=builder /app/simple-http-echo-server /usr/local/bin/simple-http-echo-server

# Switch to non-root user
USER appuser

# Set the entrypoint
ENTRYPOINT ["simple-http-echo-server"]
