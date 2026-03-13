# Use the official Rust image as the build stage
FROM rust:1.83 as builder

# Create a new empty shell project
WORKDIR /usr/src/arbitroBot
COPY Cargo.toml Cargo.lock* ./

# Copy the source code
COPY src ./src

# Build for release
RUN cargo build --release

# Use a minimal Ubuntu image for the runtime
FROM ubuntu:22.04

# Install required dependencies (OpenSSL is needed for reqwest/websockets)
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/arbitroBot/target/release/arbit_hub /usr/local/bin/arbit_hub

# Entry point
CMD ["arbit_hub"]
