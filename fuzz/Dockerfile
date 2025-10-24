# Dockerfile for running honggfuzz on Linux
FROM rust:1.82-bookworm

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    binutils-dev \
    libunwind-dev \
    libblocksruntime-dev \
    && rm -rf /var/lib/apt/lists/*

# Install honggfuzz
RUN cargo install honggfuzz --version 0.5.58

# Set working directory
WORKDIR /workspace

# Copy the project
COPY . .

# Default command
CMD ["cargo", "hfuzz", "run", "fuzz_deposit"]
