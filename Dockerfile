FROM rust:1.85-alpine AS builder
# FROM rust:1.85-slim AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY Cargo.* ./
COPY src src
RUN cargo build --release

FROM gcr.io/distroless/static-debian12
# FROM debian:12-slim
COPY --from=builder /app/target/release/http_loggo /usr/local/bin/http_loggo
ENTRYPOINT ["/usr/local/bin/http_loggo"]
