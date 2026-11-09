# Maku Server : relais Seerr et Tracearr (voir server/README.md).
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

# Image finale : le seul programme, sans système autour (les certificats racine sont intégrés).
FROM scratch
COPY --from=build /src/target/release/maku-server /maku-server
USER 65532:65532
EXPOSE 32600
ENTRYPOINT ["/maku-server"]
