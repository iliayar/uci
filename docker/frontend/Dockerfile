FROM rustlang/rust:nightly AS builder

# Install dependencies
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    nodejs \
    npm \
    curl \
    wget \
    git \
    && rm -rf /var/lib/apt/lists/*

# Install node 20.x
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get update && \
    apt-get install -y nodejs && \
    rm -rf /var/lib/apt/lists/*

# Install tailwindcss
RUN npm install --global tailwindcss @tailwindcss/cli

# Install latest Rust nightly with wasm32 target
RUN rustup target add wasm32-unknown-unknown

# Install trunk
RUN cargo install trunk

WORKDIR /usr/src/uci

# Copy project files
COPY Cargo.toml Cargo.lock ./
COPY models models/
COPY runner_client runner_client/
COPY webgui webgui/

WORKDIR /usr/src/uci/webgui

# Runtime environment variables to be replaced on startup
ENV UCI_BASE_URL=http://backend:3002
ENV UCI_WS_BASE_URL=ws://backend:3002

# Build the application
RUN trunk build --release

# Nginx stage
FROM nginx:alpine

WORKDIR /usr/share/nginx/html

# Copy built files from builder stage
COPY --from=builder /usr/src/uci/webgui/dist /usr/share/nginx/html

# Copy custom nginx configuration
COPY docker/frontend/nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

# Use a startup script to replace environment variables
COPY docker/frontend/docker-entrypoint.sh /docker-entrypoint.sh
RUN chmod +x /docker-entrypoint.sh

ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["nginx", "-g", "daemon off;"]
