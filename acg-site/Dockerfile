FROM rust:1.72.1 as builder

WORKDIR /usr/src/app
COPY amq-types ./amq-types
COPY database ./database
# avoid copying static files
RUN mkdir -p acg-site

WORKDIR /usr/src/app/acg-site
COPY acg-site/src ./src
COPY acg-site/queries ./queries
COPY ["acg-site/Cargo.lock", "acg-site/Cargo.toml", "./"]

# Will build and cache the binary and dependent crates in release mode
RUN --mount=type=cache,target=/usr/local/cargo,from=rust:1.72.1,source=/usr/local/cargo \
    --mount=type=cache,target=target \
    cargo build --release && mv ./target/release/acg-site-app ../site-app

COPY acg-site/public ./public

# Runtime image
FROM debian:bookworm-slim

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
