# syntax=docker/dockerfile:1

# Build stage
FROM oven/bun:1 AS builder

WORKDIR /app

# Copy package files
COPY package.json bun.lock ./

# Install dependencies
RUN bun install --frozen-lockfile

# Copy source files
COPY . .

# Build the static export
RUN bun run build

# Runtime stage - serve static files with nginx
FROM nginx:alpine

# Copy static export to nginx
COPY --from=builder /app/out /usr/share/nginx/html

# Copy nginx config for SPA routing with dynamic route fallbacks
RUN echo 'server { \
    listen 3000; \
    root /usr/share/nginx/html; \
    index index.html; \
    \
    # Handle movie detail pages - /movies/{id} falls back to /movies/_/index.html \
    location ~ ^/movies/[^/]+/?$ { \
        try_files $uri $uri/ /movies/_/index.html; \
    } \
    \
    # Handle TV show detail pages - /tv/{id} falls back to /tv/_/index.html \
    location ~ ^/tv/[^/]+/?$ { \
        try_files $uri $uri/ /tv/_/index.html; \
    } \
    \
    # Handle album detail pages - /music/albums/{id} falls back to /music/albums/_/index.html \
    location ~ ^/music/albums/[^/]+/?$ { \
        try_files $uri $uri/ /music/albums/_/index.html; \
    } \
    \
    # Handle artist detail pages - /music/artists/{id} falls back to /music/artists/_/index.html \
    location ~ ^/music/artists/[^/]+/?$ { \
        try_files $uri $uri/ /music/artists/_/index.html; \
    } \
    \
    # Default SPA fallback for other routes \
    location / { \
        try_files $uri $uri/ /index.html; \
    } \
}' > /etc/nginx/conf.d/default.conf

# Install wget for health checks
RUN apk add --no-cache wget

EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:3000/ || exit 1

CMD ["nginx", "-g", "daemon off;"]
