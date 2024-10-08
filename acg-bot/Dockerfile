FROM rust:1.72.1 as builder

RUN apt-get update
RUN apt-get install -y libopus-dev

WORKDIR /usr/src/app

COPY amq-types ./amq-types
COPY database ./database
COPY song-artist ./song-artist
COPY stream-song ./stream-song
COPY chokotan-bot ./chokotan-bot
COPY acg-bot ./acg-bot

WORKDIR /usr/src/app/acg-bot

# Will build and cache the binary and dependent crates in release mode
RUN --mount=type=cache,target=/usr/local/cargo,from=rust:1.72.1,source=/usr/local/cargo \
    --mount=type=cache,target=target \
    cargo build --release && mv ./target/release/acg-bot ./bot

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update
RUN apt-get install -y ca-certificates
RUN apt-get install -y libopus-dev
# i think i have to do this now because
# i'm using rust bookworm image
# and bullseye has a different version of openssl?
RUN apt-get install -y libssl-dev

# Run as "app" user
RUN useradd -ms /bin/bash app

RUN mkdir -p /app-data && chown -R app /app-data

USER app
WORKDIR /app

# Get compiled binaries from builder's cargo install directory
COPY --from=builder /usr/src/app/acg-bot/bot /app/bot
# Copy the static files
COPY --from=builder /usr/src/app/acg-bot/public /app/public


# Run the app
CMD ["./bot"]
