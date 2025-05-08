FROM rust:1.72 as builder

WORKDIR /usr/src/evolveme
COPY . .

# Build the application with release profile
RUN cargo build --release

# Runtime image
FROM debian:bullseye-slim

# Install required dependencies for SSL support
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /usr/src/evolveme/target/release/evolveme-backend /usr/local/bin/evolveme-backend

# Copy the migrations folder
COPY --from=builder /usr/src/evolveme/migrations /usr/local/bin/migrations

# Set the working directory
WORKDIR /usr/local/bin

# Expose the port
EXPOSE 8080

# Command to run the binary
CMD ["evolveme-backend"]