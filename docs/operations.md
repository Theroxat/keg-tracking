# Operations

By the end of this page you can update, back up, restore, check and troubleshoot a running kegtrack.

All commands run with `docker compose run --rm app <command>`; the list is in [Configuration](configuration.md#commands).

## Update kegtrack

```bash
git pull
docker compose up -d --build
```

Migrations run automatically at startup, before the port opens. A PostgreSQL advisory lock stops two instances from running them at the same time. Take a [backup](#back-up) first.

## Back up

The `sauvegarde` service writes a `pg_dump --format=custom` archive at startup and then every night at 03:15, with rotation: 7 daily, 4 weekly, 6 monthly (set with `RETENTION_*`).

Two safeguards, because the two classic silent failures are archiving a database not yet migrated and archiving an empty one without noticing:

1. the first backup **waits** for the schema to exist;
2. every archive is read back (`pg_restore --list`) and rejected if it does not contain the data of `evenement_fut`.

```bash
docker compose exec sauvegarde ls -lh /sauvegardes/quotidien
docker compose logs sauvegarde --tail 20
```

Archives live in the `sauvegardes` volume on the same host. Copy them elsewhere, and keep your `.env` with them.

## Restore

A backup never restored is not a backup. This one is replayed by continuous integration on every change, and here is the manual version.

**Rehearsal without risk**, in a throwaway database:

```bash
DUMP=$(docker compose exec -T sauvegarde sh -c 'ls -1t /sauvegardes/quotidien/*.dump | head -1' | tr -d '\r')

docker compose exec -T postgres psql -U postgres -d postgres \
  -c "drop database if exists webqr_restaure with (force);" \
  -c "create database webqr_restaure;"

docker compose exec -T sauvegarde \
  pg_restore -h postgres -U postgres --dbname=webqr_restaure \
             --no-owner --no-privileges "$DUMP"

docker compose exec -T postgres psql -U postgres -d webqr_restaure -tAc \
  "select (select count(*) from fut) || ' kegs, ' ||
          (select count(*) from evenement_fut) || ' events'"
```

Compare with the live database, then `drop database webqr_restaure`.

**Real restore**, after an incident:

```bash
docker compose stop app caddy
docker compose exec -T postgres psql -U postgres -d postgres \
  -c "drop database webqr with (force);" -c "create database webqr owner webqr_owner;"
docker compose exec -T sauvegarde pg_restore -h postgres -U postgres --dbname=webqr "$DUMP"
docker compose start app caddy
docker compose run --rm app reconstruire-projections --verifier
```

The last line is not decoration: it proves the restored database is consistent, by replaying the whole event stream and comparing it with the current state.

## Check projections

The state of a keg shown in lists is a **cache**. The truth is the append-only sequence of its events.

```bash
# Checks without writing anything; exits with an error if there is a discrepancy.
docker compose run --rm app reconstruire-projections --verifier

# Recomputes and fixes.
docker compose run --rm app reconstruire-projections --locataire phare
```

Run it after a restore, after fixing a projection bug, or simply to be sure. Replay is idempotent.

## Export or delete a brewery

```bash
docker compose run --rm app exporter-locataire --locataire phare --sortie phare.json
docker compose run --rm app supprimer-locataire --locataire phare --confirmer
```

The export holds every table of the tenant as open JSON. Deletion is permanent and cascades to everything. It is recorded in the publisher audit log.

## Rotate the application secret

`SECRET_APP` peppers Argon2id. **Changing it invalidates all passwords and all PIN codes.** If you must:

1. warn the users;
2. change the variable and restart;
3. reset every password from *Administration → Users → Reset*.

There is no gentle migration. That is the price of the pepper, and it is deliberate.

## Watch it

Structured logs go to standard output, level set by `RUST_LOG` (for example `info,web=debug,sqlx=warn`).

| Point | Use |
|---|---|
| `GET /sante` | Liveness: the process answers |
| `GET /pret` | Readiness: the database answers too (used by the `healthcheck`) |

The two audit logs are described in [Security](security.md#audit-logs).

## Troubleshoot

| Symptom | Check |
|---|---|
| The app does not start | `docker compose logs app`: it lists every missing variable at once |
| The camera shows nothing | The page is not served over HTTPS: see [HTTPS and camera](https-camera.md) |
| Everyone is locked out after a change | `SECRET_APP` changed: reset the passwords |
| Lists disagree with the timeline | Run `reconstruire-projections --verifier` |

## Scale

!!! warning "Single instance"
    The v1 deployment is a single instance. Rate limiting per IP lives in memory.

To run several application instances behind the same proxy, one point needs work: move per-IP rate limiting to the database or a shared cache. Nothing else: sessions, capture drafts and reference counters already live in the database.

PostgreSQL stays the only state. Indexes and server-side pagination are sized for a fleet of several thousand kegs and several hundred thousand events.
