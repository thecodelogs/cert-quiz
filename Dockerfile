# syntax=docker/dockerfile:1

FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Build dependencies first so they're cached across source-only changes.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# sqlx::migrate!() embeds ./migrations at compile time, so it has to be
# present for this build, even though the runtime image never reads it.
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --shell /usr/sbin/nologin drill

COPY --from=builder /app/target/release/drill ./drill
COPY templates ./templates
COPY static ./static

USER drill
EXPOSE 3000
CMD ["./drill"]
