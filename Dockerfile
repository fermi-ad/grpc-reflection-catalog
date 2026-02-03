# Stage 1: Build
FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev gcc git curl tar

WORKDIR /app
COPY . .

RUN cargo build --release

# Stage 2: Runtime
FROM alpine:3

WORKDIR /app
COPY --from=builder /app/target/release/grpc-reflection-catalog /app/reflection

ENV PROTO_PATH=/etc/protos/interface-definitions/proto

USER 1000
EXPOSE 50051

ENTRYPOINT ["/app/reflection"]

