FROM rust:1.77-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

COPY Cargo.toml ./

RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src \
    && rm -rf target/release/tiantan*

COPY src/ src/
COPY config/ config/

RUN cargo build --release

FROM alpine:3.19

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/tiantan-acoustics-backend /usr/local/bin/
COPY config/ /app/config/

EXPOSE 8080 9090

ENV CLICKHOUSE_URL=http://clickhouse:8123
ENV MQTT_BROKER=mqtt://mqtt-broker:1883

CMD ["tiantan-acoustics-backend"]
