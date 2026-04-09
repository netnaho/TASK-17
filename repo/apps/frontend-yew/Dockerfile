FROM rust:latest AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates curl && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk@0.21.14
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY crates ./crates
COPY migrations ./migrations
COPY unit_tests ./unit_tests
WORKDIR /build/apps/frontend-yew
RUN trunk build --release

FROM nginx:1.27-alpine
COPY --from=builder /build/apps/frontend-yew/dist /usr/share/nginx/html
# SPA fallback config: all unknown paths serve index.html for yew-router
COPY apps/frontend-yew/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
