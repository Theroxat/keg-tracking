#!/bin/bash
# Outillage de développement : tout se compile dans un conteneur, la machine
# hôte n'a pas besoin d'une chaîne Rust.
set -euo pipefail
cd "$(dirname "$0")"
set -a; . ./.env; set +a
export DATABASE_URL="postgres://webqr_owner:${MDP_OWNER}@webqr_pg_dev:5432/webqr"
exec docker run --rm --network webqr_dev \
  -v "$PWD":/w -w /w \
  -v webqr_cargo_registry:/usr/local/cargo/registry \
  -v webqr_target:/w/target \
  -e CARGO_TERM_COLOR=never -e DATABASE_URL -e SQLX_OFFLINE=false \
  -e MDP_OWNER -e MDP_APP -e MDP_EDITEUR -e MDP_AUTH -e SECRET_APP \
  -e SERVEUR_TEST=webqr_pg_dev:5432 \
  -e CARGO_NET_RETRY=5 \
  webqr/dev:latest "$@"
