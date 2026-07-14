FROM docker.io/library/rust:alpine AS builder

RUN apk add --no-cache cmake g++ musl-dev
COPY . .

ENV RUSTFLAGS="-C link-arg=-static-libstdc++ -C link-arg=-static-libgcc"
RUN cargo build --release

FROM scratch
COPY --from=builder /target/release/spritmonitor-stats /spritmonitor-stats
ENTRYPOINT ["/spritmonitor-stats"]



