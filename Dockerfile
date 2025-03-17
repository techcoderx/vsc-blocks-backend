# Build stage
FROM rust:latest AS builder

WORKDIR /app

# Copy source files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY lib ./lib
RUN mkdir as_compiler
COPY as_compiler/package-template.json ./as_compiler/package-template.json

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y openssl ca-certificates

# Copy built binary from builder
COPY --from=builder /app/target/release/vsc-contract-verifier /app/vsc-contract-verifier

# Set working directory and default command
WORKDIR /app
EXPOSE 8080
CMD ["/app/vsc-contract-verifier", "-c", "/app/config/config.toml"]