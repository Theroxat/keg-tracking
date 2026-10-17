# Security Policy

## Supported versions

Only the latest `main` is supported.

## Reporting a vulnerability

Please do not open a public issue. Use GitHub's private vulnerability reporting: on the repository, go to Security, then "Report a vulnerability". Include steps to reproduce and the impact.

## Scope notes

- Tenant isolation is a core guarantee: PostgreSQL Row Level Security applies to every table with a `tenant_id` column. Any way to read or write another brewery's data is in scope and treated as critical.
- The event stream (`evenement_fut`) is append-only. Any way to alter or delete events is in scope.
- Never commit `.env` or any real password. `SECRET_APP` is the Argon2id pepper: changing it invalidates all passwords and PINs.
- The camera requires HTTPS, and the CSP is strict (`default-src 'self'`, no inline JavaScript). Serve the application only behind TLS.
