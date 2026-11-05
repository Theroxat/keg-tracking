# Configuration

By the end of this page you know every variable, command, port, volume and endpoint.

Configuration is done only through environment variables, in `.env`. If variables are missing, the application refuses to start and lists **all** of them at once.

## Environment variables

Set in `.env` and passed on by `compose.yaml`:

| Variable | Purpose | Default |
|---|---|---|
| `POSTGRES_PASSWORD` | PostgreSQL superuser (initialisation, backups) | none, required |
| `MDP_OWNER`, `MDP_APP`, `MDP_EDITEUR`, `MDP_AUTH` | Passwords of the four PostgreSQL roles | none, required |
| `SECRET_APP` | Argon2id pepper, at least 32 characters. **Changing it invalidates all passwords and PINs.** | none, required |
| `DOMAINE_PUBLIC` | Served domain, for example `qr.exemple.fr` | none, required |
| `PORT_PUBLIC` | Port exposed by Caddy | `6666` |
| `FUSEAU_DEFAUT` | Time zone | `Europe/Paris` |
| `RUST_LOG` | Log level, for example `info,web=debug,sqlx=warn` | `info` |
| `RETENTION_QUOTIDIENNE` | Daily backups kept | `7` |
| `RETENTION_HEBDOMADAIRE` | Weekly backups kept | `4` |
| `RETENTION_MENSUELLE` | Monthly backups kept | `6` |

Set by `compose.yaml` for the `app` service, read by the application:

| Variable | Purpose | Default |
|---|---|---|
| `DATABASE_URL_OWNER`, `_APP`, `_EDITEUR`, `_AUTH` | One connection URL per PostgreSQL role, built from the passwords above | none, required |
| `ADRESSE_ECOUTE` | Listen address of the application | `0.0.0.0:8080` |
| `DERRIERE_PROXY` | Read the real client address from the headers sent by the proxy | `true` |
| `COOKIES_SECURISES` | The browser sees HTTPS even if the app receives HTTP (behind a tunnel). Cookies stay `Secure`. | `true` |
| `CONNEXIONS_MAX` | Maximum database connections | `16` |

## Commands

All run with `docker compose run --rm app <command>`.

| Command | Effect |
|---|---|
| `serve` | Starts the server. This is the default command of the container. |
| `provisionner-brasserie` | Creates a brewery, its first administrator and its base locations |
| `creer-editeur` | Creates a publisher back-office account |
| `semer-demonstration` | Injects a demonstration fleet with one year of history |
| `reconstruire-projections [--verifier]` | Replays the event stream. With `--verifier`, detects discrepancies without writing anything. |
| `exporter-locataire` | Exports all data of a brewery as JSON |
| `supprimer-locataire` | Permanently deletes a brewery |

Use cases for each are in [Install kegtrack](installation.md) and [Operations](operations.md).

## Services, ports and volumes

| Service | Image | Port | Role |
|---|---|---|---|
| `caddy` | `caddy:2-alpine` | `PORT_PUBLIC` (default `6666`) | Reverse proxy, compression, headers |
| `app` | built from `docker/Dockerfile` | `8080` (internal) | The `webqr` binary |
| `postgres` | `postgres:17-alpine` | `5432` (internal) | The only state |
| `sauvegarde` | `postgres:17-alpine` | none | Nightly `pg_dump` with rotation |

| Volume | Holds |
|---|---|
| `pg_data` | The PostgreSQL data |
| `sauvegardes` | The backup archives |
| `caddy_data` | Caddy certificates |
| `caddy_config` | Caddy configuration |

## Endpoints for monitoring

| Endpoint | Meaning |
|---|---|
| `GET /sante` | Liveness: the process answers. Answered by Caddy. |
| `GET /pret` | Readiness: the database answers too. Used by the container healthcheck. |
