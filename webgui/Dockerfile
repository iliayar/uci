FROM ubuntu:latest

RUN apt-get update && \
    apt-get install -y curl wget

RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get install -y nodejs

RUN npm install --global tailwindcss

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /rustup.sh && \
    chmod +x /rustup.sh

RUN /rustup.sh -y --default-toolchain nightly -t wasm32-unknown-unknown

RUN wget -qO- https://github.com/thedodd/trunk/releases/download/v0.17.2/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf-

WORKDIR /build

