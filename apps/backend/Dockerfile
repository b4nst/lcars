# syntax=docker/dockerfile:1

# Build stage
FROM rust:1.92-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Create dummy src to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs

# Build dependencies only (cached layer)
RUN cargo build --release && rm -rf src target/release/deps/backend*

# Copy actual source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    ffmpeg \
    wget \
    && rm -rf /var/cache/apk/*

# Create non-root user
RUN addgroup -g 1000 lcars && \
    adduser -u 1000 -G lcars -s /bin/sh -D lcars

# Copy the binary
COPY --from=builder /app/target/release/backend /usr/local/bin/lcars-backend

# Create directories
RUN mkdir -p /data /downloads /media && \
    chown -R lcars:lcars /data /downloads /media

USER lcars

# Expose port
EXPOSE 8080

# Volumes
VOLUME ["/data", "/downloads", "/media"]

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# Set environment variables
ENV LCARS_DATABASE__PATH=/data/lcars.db \
    LCARS_TORRENT__DOWNLOAD_DIR=/downloads \
    LCARS_SERVER__HOST=0.0.0.0

ENTRYPOINT ["lcars-backend"]
