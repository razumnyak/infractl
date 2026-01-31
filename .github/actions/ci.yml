# Multi-stage Dockerfile for infractl
# Optimized for minimal image size

# =============================================================================
# Stage 1: Build (if building from source)
# =============================================================================
FROM rust:1.84-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconf

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy main to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source
COPY src ./src
COPY assets ./assets
COPY build.rs ./

# Build final binary
RUN touch src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl && \
    strip target/x86_64-unknown-linux-musl/release/infractl

# =============================================================================
# Stage 2: Runtime (minimal)
# =============================================================================
FROM alpine:3.21

# Labels
LABEL org.opencontainers.image.title="infractl"
LABEL org.opencontainers.image.description="Infrastructure monitoring and deployment agent"
LABEL org.opencontainers.image.source="https://github.com/your-org/infractl"

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    tzdata \
    docker-cli \
    docker-cli-compose \
    git \
    openssh-client

# Create non-root user
RUN addgroup -S infractl && \
    adduser -S -G infractl infractl && \
    mkdir -p /var/lib/infractl /var/log/infractl /etc/infractl && \
    chown -R infractl:infractl /var/lib/infractl /var/log/infractl /etc/infractl

# Copy binary from builder
# Option A: From builder stage
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/infractl /usr/local/bin/

# Option B: From pre-built artifact (uncomment if using CI artifacts)
# COPY dist/infractl /usr/local/bin/

# Make executable
RUN chmod +x /usr/local/bin/infractl

# Default config
COPY config/default.yaml /etc/infractl/config.yaml

# Expose port
EXPOSE 8111

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8111/health || exit 1

# Run as non-root (comment out if Docker socket access needed as root)
# USER infractl

# Volumes
VOLUME ["/var/lib/infractl", "/etc/infractl"]

# Entry point
ENTRYPOINT ["/usr/local/bin/infractl"]
CMD ["--config", "/etc/infractl/config.yaml"]