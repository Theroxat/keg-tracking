# Development

By the end of this page you can build kegtrack, run its tests and send a change.

Everything builds in a container. `./dev.sh` loads `.env`, mounts the repository in the `webqr/dev:latest` image and connects to the development database.

## Set up

```bash
git clone https://github.com/selfhostr/kegtrack.git && cd kegtrack
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

## Run the checks

```bash
./dev.sh cargo test --workspace                          # full suite
./dev.sh cargo test -p web --test acces                  # a single test binary
./dev.sh cargo clippy --workspace --all-targets -- -D warnings
./dev.sh cargo fmt --all --check
./dev.sh cargo sqlx prepare --workspace -- --all-targets # after any modified query
```

## Tests

Each test gets a **fresh, migrated PostgreSQL database** with the real production roles. Without that, the RLS tests would prove nothing.

| Binary | What it proves |
|---|---|
| `isolation` | Six attack scenarios against tenant separation, on tables enumerated from the catalogue |
| `acces` | Every route with every role, plus CSRF, missing session and brewery-scoped global search |
| `documents` | Atomicity, deduplication, early refusal, draft survival |
| `coherence` | An address belonging to another customer is refused by the server **and** by the database |
| `reconstruction` | Replay yields the current state, and a corrupted projection is detected |
| `proprietes` | State-machine invariants over thousands of generated histories |

## Continuous integration

The workflow `.github/workflows/integration.yml` runs on every push to `main` and every pull request:

- format, clippy, offline SQLx query check and tests;
- projection replay on a demonstration dataset;
- QR decoding of a generated label sheet;
- no horizontal overflow on iPhone 13, Pixel 5 and 320 px (Playwright);
- dependency audit (`cargo deny`, `cargo audit`);
- Docker image build;
- backup then restore exercise.

## Rules for contributors

| If you… | Then also… |
|---|---|
| add a route | register it with its role in `web::routes::INVENTAIRE` (otherwise a test fails) |
| change an SQLx query | rerun `cargo sqlx prepare` (CI checks with `--check`) and commit the `.sqlx` changes |
| add a business table | give it a `tenant_id` column and end the migration with `select appliquer_rls();` |
| add a variant of `Statut`, `Role`, `TypeEvenement` or `TypeDocument` | update `TOUS`, `cle_i18n()` and `crates/web/src/i18n/fr.rs` and `en.rs` |
| add user-visible text | go through `t!("key")` (a missing key is a compile error) |
| add JavaScript behaviour | no `onclick` or inline `<script>` (the CSP blocks them): use a `data-*` attribute handled by `assets/js/commun.js` |
| add a static asset | put it in `assets/` and embed it in `crates/web/src/statique.rs`. No CDN: the CSP is `default-src 'self'`. |
| add a setting | read it from an environment variable in `web::Config::depuis_env`, never hard-code a secret |

!!! note "Language"
    The project is written in French: identifiers, comments and commit messages. Only infrastructure (Docker, CI) and third-party names stay in English. GitHub-facing files and these docs are in English, with French translations next to them (`page.fr.md`).

See also [CONTRIBUTING.md](https://github.com/selfhostr/kegtrack/blob/main/CONTRIBUTING.md).
