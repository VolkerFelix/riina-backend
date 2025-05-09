# Stage 1: Generate recipe file
FROM rust:1.84 AS chef
RUN cargo install cargo-chef
WORKDIR /app

# Stage 2: Prepare build cache
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build dependencies - this is cached unless dependencies change
FROM chef AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies only
RUN cargo chef cook --release --recipe-path recipe.json
# Install sqlx-cli for migrations
RUN cargo install sqlx-cli --no-default-features --features postgres

# Stage 4: Build application - this only rebuilds your actual code
COPY . .
RUN cargo build --release

# Stage 5: Runtime environment
FROM debian:bookworm-slim

RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

# Create app directory for config
WORKDIR /app
RUN mkdir -p configuration
COPY --from=builder /app/configuration/ /app/configuration/

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/areum-backend /usr/local/bin/areum-backend

# Copy migrations and startup script
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY scripts/migrate_and_run.sh /app/migrate_and_run.sh
RUN chmod +x /app/migrate_and_run.sh

ENV APP_ENVIRONMENT=production
# Set the entry point
CMD ["/app/migrate_and_run.sh"]