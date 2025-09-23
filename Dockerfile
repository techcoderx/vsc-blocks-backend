# Build stage
FROM rust:latest AS builder

WORKDIR /app

# Download Wabt and Wasm Tools
RUN wget https://github.com/WebAssembly/wabt/releases/download/1.0.37/wabt-1.0.37-ubuntu-20.04.tar.gz && \
  tar -xvf wabt-1.0.37-ubuntu-20.04.tar.gz
RUN wget https://github.com/bytecodealliance/wasm-tools/releases/download/v1.239.0/wasm-tools-1.239.0-x86_64-linux.tar.gz && \
  tar -xvf wasm-tools-1.239.0-x86_64-linux.tar.gz

# Copy source files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY lib ./lib

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:trixie-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y openssl ca-certificates curl && rm -rf /var/lib/apt/lists/*

# Install Wabt and Wasm Tools
COPY --from=builder /app/wabt-1.0.37/bin/wasm-strip /usr/bin
COPY --from=builder /app/wasm-tools-1.239.0-x86_64-linux/wasm-tools /usr/bin

# Copy built binary from builder
COPY --from=builder /app/target/release/vsc-blocks-backend /app/vsc-blocks-backend

# Set working directory and default command
WORKDIR /app
EXPOSE 8080
CMD ["/app/vsc-blocks-backend", "-c", "/app/config/config.toml"]