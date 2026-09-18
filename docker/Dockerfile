# syntax=docker/dockerfile:1

# ---------------------------------------------------------------- planificateur
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /w

FROM chef AS planificateur
COPY . .
RUN cargo chef prepare --recipe-path recette.json

# ---------------------------------------------------------------- compilation
FROM chef AS constructeur
COPY --from=planificateur /w/recette.json recette.json
# Les dépendances se compilent une fois et restent en cache tant que le
# manifeste ne bouge pas.
RUN cargo chef cook --release --recipe-path recette.json
COPY . .
# Les requêtes sont vérifiées hors ligne, contre les métadonnées déposées
# dans .sqlx par `cargo sqlx prepare` : la construction de l'image n'a pas
# besoin d'accéder à une base.
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin webqr && strip target/release/webqr

# ---------------------------------------------------------------- exécution
FROM debian:bookworm-slim AS execution
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl tzdata \
 && rm -rf /var/lib/apt/lists/*
# Aucun privilège superflu : le service tourne sous un compte dédié.
RUN useradd --system --uid 10001 --create-home --home-dir /app webqr
WORKDIR /app
COPY --from=constructeur /w/target/release/webqr /usr/local/bin/webqr
USER webqr
ENV ADRESSE_ECOUTE=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=3s --start-period=40s --retries=5 \
  CMD curl -fsS http://127.0.0.1:8080/pret || exit 1
ENTRYPOINT ["webqr"]
CMD ["serve"]
