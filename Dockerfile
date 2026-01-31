# Multi-stage Dockerfile for infractl
# Optimized for minimal image size

# =============================================================================
# Stage 1: Build
# =============================================================================
FROM rust:1.84-alpine AS builder

RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconf \
    git \
    perl \
    make

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy main to build dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source
COPY src ./src

# Build final binary
RUN touch src/main.rs && \
    cargo build --release && \
    strip target/release/infractl

# =============================================================================
# Stage 2: Runtime (minimal)
# =============================================================================
FROM alpine:3.21

# Labels
LABEL org.opencontainers.image.title="infractl"
LABEL org.opencontainers.image.description="Infrastructure monitoring and deployment agent"
LABEL org.opencontainers.image.source="https://github.com/razumnyak/infractl"
LABEL org.opencontainers.image.version="0.1.0"

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    tzdata \
    docker-cli \
    docker-cli-compose \
    git \
    openssh-client \
    curl

# Create directories
RUN mkdir -p /var/lib/infractl /var/log/infractl /etc/infractl && \
    chmod 755 /var/lib/infractl /var/log/infractl /etc/infractl

# Copy binary from builder
COPY --from=builder /app/target/release/infractl /usr/local/bin/

# Make executable
RUN chmod +x /usr/local/bin/infractl

# Copy default config
COPY scripts/docker-config.yaml /etc/infractl/config.yaml

# Expose port
EXPOSE 8111

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -sf http://localhost:8111/health || exit 1

# Volumes
VOLUME ["/var/lib/infractl", "/etc/infractl"]

# Entry point
ENTRYPOINT ["/usr/local/bin/infractl"]
CMD ["--config", "/etc/infractl/config.yaml"]
