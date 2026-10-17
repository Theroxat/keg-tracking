# Contributing

Issues and pull requests are welcome on [GitHub](https://github.com/Theroxat/WebQr_brasserie). For an architecture overview and the rules the code relies on, read [CLAUDE.md](CLAUDE.md) and the Architecture section of the [README](README.md).

The project is written in French: domain identifiers, code comments, documentation and **commit messages** stay in French. Only infrastructure (Docker, CI) and third-party names are in English. GitHub-facing files (README, issues, pull requests) are in English.

## Setup

Everything builds in a container, so no Rust toolchain is needed on the host. `./dev.sh` loads `.env`, mounts the repository in the `webqr/dev:latest` image and connects it to the development database.

```bash
git clone https://github.com/Theroxat/WebQr_brasserie.git && cd WebQr_brasserie
cp .env.exemple .env   # fill every password
docker build -f docker/Dockerfile.dev -t webqr/dev:latest .
docker network create webqr_dev
docker run -d --name webqr_pg_dev --network webqr_dev \
  -e POSTGRES_DB=webqr -e POSTGRES_PASSWORD=… \
  -e MDP_OWNER=… -e MDP_APP=… -e MDP_EDITEUR=… -e MDP_AUTH=… \
  -v "$PWD/docker/initdb":/docker-entrypoint-initdb.d:ro \
  -v webqr_pgdev:/var/lib/postgresql/data postgres:17-alpine
```

The passwords must match those in `.env`.

## Checks

CI runs these, and a change must pass them:

```bash
./dev.sh cargo fmt --all --check
./dev.sh cargo clippy --workspace --all-targets -- -D warnings
./dev.sh cargo test --workspace
./dev.sh cargo sqlx prepare --workspace -- --all-targets   # after any modified query
```

## Testing

Each integration test gets a fresh, migrated PostgreSQL database with the real production roles, so the isolation tests prove something. Add tests for any behaviour you change, and extend the existing structural tests rather than working around them.

## Rules

| If you... | Then also... |
|---|---|
| add a route | register it with its role in `web::routes::INVENTAIRE` (a test fails otherwise) |
| change an SQLx query | rerun `cargo sqlx prepare` and commit the `.sqlx` changes |
| add a business table | give it a `tenant_id` column and end the migration with `select appliquer_rls();` |
| add a variant of `Statut`, `Role`, `TypeEvenement` or `TypeDocument` | update `TOUS`, `cle_i18n()` and the catalogues `crates/web/src/i18n/fr.rs` **and** `en.rs` |
| add user-visible text | use `t!("key")`, never a literal, and add the key to both `fr.rs` and `en.rs` |
| add JavaScript behaviour | no `onclick` and no inline `<script>` (the CSP blocks them): use a `data-*` attribute handled by `assets/js/commun.js` |
| add a static asset | put it in `assets/` and embed it in `crates/web/src/statique.rs`. No CDN: the CSP is `default-src 'self'` |
| add a setting | read it from an environment variable in `web::Config::depuis_env`, never hard-code a secret |

Only `persistance::evenement` writes to the `fut` table, and `Depot::ouvrir` is the only way to get a tenant-scoped connection. Do not bypass either.

## Pull requests

Keep them focused, one change each, and update the docs when behaviour or configuration changes. Write the commit message in French, as a short imperative summary.

Never commit `.env`, a real secret, hostname or IP.
