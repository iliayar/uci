FROM rust:slim-bullseye AS builder

WORKDIR /usr/src/uci

# Install dependencies
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy actual source code
COPY Cargo.toml Cargo.lock ./
COPY common common/
COPY models models/
COPY worker_lib worker_lib/
COPY uci_cli uci_cli/
COPY runner_lib runner_lib/
COPY runner_client runner_client/
COPY dynconf dynconf/
COPY uci uci/

# Build the application
RUN cargo build --bin ucid --release

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    libssl-dev \
    curl \
    gnupg \
    && rm -rf /var/lib/apt/lists/*

# Install Docker client
RUN mkdir -p /etc/apt/keyrings && \
    curl -fsSL https://download.docker.com/linux/debian/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/debian bullseye stable" > /etc/apt/sources.list.d/docker.list && \
    apt-get update && \
    apt-get install -y docker-ce-cli && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled binary
COPY --from=builder /usr/src/uci/target/release/ucid /usr/local/bin/ucid

# Create artifact storage directory
RUN mkdir -p /app/artifacts

EXPOSE 3002

ENV RUST_LOG=debug
ENV ARTIFACTS_DIR=/app/artifacts

CMD ["ucid", "--config", "/app/config/config.yaml"]
