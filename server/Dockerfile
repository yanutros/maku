# Maku Server : relais Seerr, Tracearr et Kavita (voir server/README.md).
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked
# Dossier de données (comptes Kavita), à l'utilisateur du relais.
RUN mkdir /data && chown 65532:65532 /data

# Image finale : le seul programme, sans système autour (les certificats racine sont intégrés).
FROM scratch
COPY --from=build /src/target/release/maku-server /maku-server
COPY --from=build --chown=65532:65532 /data /data
VOLUME /data
USER 65532:65532
EXPOSE 32700
ENTRYPOINT ["/maku-server"]
