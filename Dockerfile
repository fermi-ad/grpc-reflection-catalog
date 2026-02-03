# Stage 1: Build
FROM adregistry.fnal.gov/dev-containers/rust:1.92.0 AS builder
RUN apt-get update && apt-get install -y protobuf-compiler
WORKDIR /app
COPY . .
RUN cargo build --release

# Stage 2: Runtime
FROM gcr.io/distroless/cc-debian12

WORKDIR /app

# Copy binary from build step
COPY --from=builder /app/target/release/grpc-reflection-catalog /app/reflection

# Copy protoc (needed for runtime proto compilation)
COPY --from=builder /usr/bin/protoc /usr/bin/protoc
COPY --from=builder /usr/include/google /usr/include/google

# Distroless cc-debian12 includes many, but we should verify protoc can run.
# Since protoc is a C++ app, it needs the C++ standard library.
COPY --from=builder /usr/lib/x86_64-linux-gnu/libstdc++.so.6 /usr/lib/x86_64-linux-gnu/
COPY --from=builder /usr/lib/x86_64-linux-gnu/libgcc_s.so.1 /usr/lib/x86_64-linux-gnu/

# Set environment for the binary to find protoc
ENV PROTOC=/usr/bin/protoc
ENV PROTO_PATH=/etc/protos/interface-definitions/proto

USER 1000

EXPOSE 50051
ENTRYPOINT ["/app/reflection"]

