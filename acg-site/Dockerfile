FROM rust:latest as builder

WORKDIR /usr/src/app
COPY acg-amqtypes ./acg-amqtypes
COPY acg-database ./acg-database
# avoid copying static files
RUN mkdir -p acg-site

WORKDIR ./acg-site
COPY acg-site/acg-site-app ./acg-site-app
COPY ["acg-site/Cargo.lock", "acg-site/Cargo.toml", "."]

# Will build and cache the binary and dependent crates in release mode
RUN --mount=type=cache,target=/usr/local/cargo,from=rust:latest,source=/usr/local/cargo \
    --mount=type=cache,target=target \
    cargo build --release && mv ./target/release/acg-site-app ../site-app

COPY acg-site/public ./public

# Runtime image
FROM debian:bullseye-slim

# Run as "app" user
RUN useradd -ms /bin/bash app

USER app
WORKDIR /app

# Get compiled binaries from builder's cargo install directory
COPY --from=builder /usr/src/app/site-app /app/site-app
# Copy the static files
COPY --from=builder /usr/src/app/acg-site/public /app/public

# Run the app
CMD ["./site-app"]
