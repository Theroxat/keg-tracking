# Architecture

By the end of this page you know how kegtrack is built and why.

**Rust** (Axum and SQLx) on **PostgreSQL 17**, server-rendered with **Maud**, **htmx** for management screens, and three standalone JavaScript files: `scan/scan.js` (scanning), `pin.js` (PIN keypad) and `commun.js` (confirmations, printing, the `/` shortcut, htmx redirect on 401/403). The CSP allows no inline JavaScript. No SPA, no front-end build chain, no CDN.

## Crates

| Crate | Role |
|---|---|
| `domaine` | Statuses, transitions, projections. No Axum, no runtime, no I/O. |
| `persistance` | SQLx, embedded migrations, `Depot` and the isolation guard |
| `web` | Routes, Maud templates, i18n, middleware |
| `app` | The `webqr` binary: `serve` and the operations commands |

Dependencies only go one way:

```
app --> web --> persistance --> domaine
```

## Deployment

```
Browser --https--> TLS tunnel / proxy --> Caddy :6666 --> app (webqr serve) --> PostgreSQL 17
                                                          backup service ----> PostgreSQL 17
```

Four services: `caddy`, `app`, `postgres`, `sauvegarde`. See [Configuration](configuration.md#services-ports-and-volumes).

## Three structural guarantees

They are the reason the architecture exists, and each has its own test.

1. **Multi-tenant isolation.** PostgreSQL Row Level Security with four roles. See [Security](security.md#tenant-isolation).
2. **The event stream is the source of truth.** The `evenement_fut` table is append-only. The `fut` table is only a projection, recomputed by the same pure function (`domaine::projection::appliquer`) in real time and during a full replay, so the two paths cannot diverge. Only `persistance::evenement` writes to `fut`.
3. **Access control on every route.** See [Security](security.md#access-control).

## Technical decisions

The reasons behind the choices were written before the code, in [PHASE-0-ARCHITECTURE.md](https://github.com/selfhostr/kegtrack/blob/main/docs/PHASE-0-ARCHITECTURE.md), in French. It predates the code: on contradiction, the code wins.
