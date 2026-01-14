# 1. Build Stage
FROM rust:latest AS builder

WORKDIR /app
COPY . .

# Build release (Will now succeed because of Cargo.toml [[bin]])
RUN cargo build --release

# 2. Runtime Stage
FROM debian:bookworm-slim

# Install basics to ensure SSL connections work
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary
COPY --from=builder /app/target/release/apex_omega /app/apex_omega

# Run the binary
CMD ["./apex_omega"]
