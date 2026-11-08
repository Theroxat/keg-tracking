<div align="center">
<a href="https://kegtrack.selfhostr.io"><img src=".github/assets/banner.svg" alt="kegtrack: QR-code tracking for returnable kegs." width="100%"></a>

**A self-hosted, multi-tenant app to track returnable kegs with QR codes, for breweries and anyone who lends out deposit kegs.**

[Site](https://kegtrack.selfhostr.io) · [Docs](https://kegtrack.selfhostr.io/docs/) · [Demo](https://kegtrack.selfhostr.io/demo/) · [Changelog](CHANGELOG.md)

[![CI](https://img.shields.io/github/actions/workflow/status/selfhostr/kegtrack/integration.yml?branch=main&label=CI&logo=githubactions&logoColor=white)](https://github.com/selfhostr/kegtrack/actions/workflows/integration.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
</div>

## Overview

Every keg has a QR label and every field operation is a scan, so you always know where a keg is, what is in it, and which ones have not come back. Keg state is computed from an append-only stream of events. The interface is in English and French. Free, for good: MIT licence, no paid edition.

## Quick start

```bash
cp .env.exemple .env      # fill every password, see the installation guide
docker compose up -d --build
docker compose run --rm app provisionner-brasserie \
  --nom "Brasserie du Phare" --slug phare --prefixe-qr PHARE \
  --admin-email contact@brasserie.fr --admin-nom Martin --admin-prenom Claire
```

Needs Docker with the `compose` plugin. The last command prints the administrator password once; open the app on port `6666`, sign in, then set your PIN in *My account*. The camera requires HTTPS ([details](docs/https-camera.md)). Back up the `sauvegardes` volume and your `.env`; see [Operations](docs/operations.md).

## Features

- **Scanner.** Bluetooth scanner, rear camera with torch, or manual entry, with colour, sound and vibration feedback.
- **Batch operations.** Washing, filling, delivery and return: scan dozens of kegs, validated all or nothing.
- **Delivery note.** Generated on validation, printable, with two signature fields.
- **Fleet.** Full-text search, filters, age in status, CSV and Excel export.
- **Lots and recall.** Every customer concerned by a lot, with contacts, quantities and exact QR codes.
- **Customers.** The kegs each one holds; a customer is never deleted.
- **Administration.** Beers, locations, tanks, users, QR label sheets, spreadsheet import, audit log.
- **Roles.** Read, Production, Administrator, plus a separate publisher back-office. Mobile first, dark theme by default.

The full tour is in the [user guide](docs/user-guide.md).

## Architecture

Rust (Axum, SQLx) on PostgreSQL 17, server-rendered with Maud and htmx, no SPA and no CDN. Tenants are isolated by PostgreSQL Row Level Security, and the event table is append-only. Details in [Architecture](docs/architecture.md).

## Limits

- Out of scope for v1: billing, deposits, offline operation, a native app, self-service sign-up, a public API, multi-site.
- One instance: rate limiting per IP lives in memory.
- Paper labels do not survive an 80 °C caustic wash.
- Code, identifiers and commit messages are in French; the documentation is in English and French.

## Contributing

Issues and pull requests are welcome: [CONTRIBUTING.md](CONTRIBUTING.md). Security issues: [SECURITY.md](SECURITY.md).

## License

MIT, see [LICENSE](LICENSE). Provided "as is", without warranty of any kind.

## Support

kegtrack is free and has no company behind it. If it saves you time: [sponsor us on GitHub](https://github.com/sponsors/selfhostr).
