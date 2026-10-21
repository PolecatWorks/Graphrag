# Stage 1: Chef
FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef
WORKDIR /app

# Stage 2: Planner
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Buildcache
FROM chef AS buildcache
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 4: Dev (optional layer for dev builds if needed, keeping structure)
FROM chef AS dev
COPY . .
# You would do cargo build here if it was dev

# Stage 5: Buildcache release (just continuing from buildcache)
FROM buildcache AS buildcacherelease

# Stage 6: Build
FROM buildcacherelease AS build
COPY . .
RUN cargo build --release
# Verify execution
RUN /app/target/release/app --help

# Stage 7: Runtime
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install runtime dependencies
RUN apt-get update -y && \
    apt-get install -y --no-install-recommends libssl3 ca-certificates && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Copy binaries and configs
COPY --from=build /app/target/release/app /usr/local/bin/app
COPY migrations /app/migrations
COPY config.yaml /app/config.yaml

EXPOSE 8080 8079

ENTRYPOINT ["/usr/local/bin/app"]
CMD ["-c", "/app/config.yaml"]
