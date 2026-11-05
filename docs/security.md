# Security

By the end of this page you know what kegtrack protects, how, and what it does not cover.

To report a vulnerability, use GitHub private vulnerability reporting, as described in [SECURITY.md](https://github.com/selfhostr/kegtrack/blob/main/SECURITY.md). Only the latest `main` is supported.

## Tenant isolation

PostgreSQL Row Level Security applies to every table with a `tenant_id` column. The application role does not own the tables. `Depot::ouvrir(locataire, utilisateur)` is the only way to obtain a business connection, so a forgotten `where tenant_id = …` returns **no** rows instead of all of them.

| PostgreSQL role | Use |
|---|---|
| `webqr_owner` | Migrations only; its connection is closed afterwards |
| `webqr_app` | All business logic, subject to RLS |
| `webqr_editeur` | Back-office outside any tenant |
| `webqr_auth` | Sessions and passwords, rights granted column by column |

Any way to read or write another brewery's data is treated as critical.

## Append-only history

The `evenement_fut` table is append-only: a trigger and revoked privileges forbid any update or delete. Any way to alter or delete events is a vulnerability.

## Access control

Routes are grouped in three groups, each with a minimum role (Read, Production, Administrator). The inventory `routes::INVENTAIRE` is the single source for the access test, which tries every route with every role, plus CSRF and missing sessions. The publisher back-office is a separate scope with its own cookie and accounts table.

## Passwords, sessions and PIN

Passwords and PINs are hashed with Argon2id and peppered with `SECRET_APP`. A session lasts 30 days on a known device, locks after 15 minutes of inactivity and asks for a six-digit PIN. Five wrong PINs revoke the session. See [Rotate the application secret](operations.md#rotate-the-application-secret) before changing the pepper.

## Rate limiting

Two complementary levels:

- **Per IP address**, in memory: 12 sign-in attempts per minute, 8 for the PIN and the publisher back-office, 240 scans per minute and per user. Reset on restart, which is acceptable for what it protects.
- **Per account**, in the database: after 5 failures, exponential lock (1, 2, 4, 8… minutes, capped at 30). This one **survives a restart** and is what really protects against password stuffing.

The real client address is read from the proxy headers (`CF-Connecting-IP`, `X-Forwarded-For`) when `DERRIERE_PROXY` is `true`.

## Support access

From `/editeur`, the **Assistance** button opens a session in the chosen brewery, as its first active administrator. This session:

- lasts one hour at most;
- is marked in the database (`session.usurpee_par`);
- shows a **permanent red banner** across the application;
- is logged on opening and closing.

!!! warning "A real power"
    Use it only on a customer's request, and close the session as soon as the diagnosis is done.

## Audit logs

- `journal_audit`, per brewery: sign-ins, role changes, archiving, imports, document validations, settings changes. Available in *Administration → Audit log*.
- `journal_audit_editeur`, outside breweries: creations, suspensions, and **every opening and closing of support access**.

## Web hardening

The Content Security Policy is `default-src 'self'` with no inline JavaScript, and there is no CDN. Serve the application only behind TLS: the camera needs it anyway (see [HTTPS and camera](https-camera.md)), cookies are `Secure`, and Caddy sends `Strict-Transport-Security`.

## What it does not cover

!!! warning "Limits"
    - Backups stay on the same host until you copy them elsewhere.
    - Per-IP rate limiting is in memory and resets on restart.
    - Never commit `.env` or any real password, hostname or IP.
