# kegtrack

By the end of this page you know what kegtrack does and where to go next.

kegtrack tracks returnable kegs with QR codes. Every keg carries a label and every field operation is a scan. It is self-hosted, multi-tenant, and runs as four Docker services. A brewery is the typical user, but nothing is specific to beer.

At any moment it answers: where is this keg and for how long, what is in it (beer, lot, tank), how many clean kegs are available, which ones have not come back after N days, and who received lot 2026-017. The history of operations is the source of truth, not the keg record.

## Start here

- [Install kegtrack](installation.md): secrets, start, first brewery.
- [Make the camera work](https-camera.md): browsers block the camera without HTTPS.
- [Use it](user-guide.md): roles, scanning, batch operations, lots and recall.
- [Understand how it is built](architecture.md): crates, isolation, event stream.

## All pages

| Section | Pages |
|---|---|
| Get started | [Installation](installation.md), [HTTPS and camera](https-camera.md) |
| Using it | [User guide](user-guide.md) |
| Reference | [Configuration](configuration.md), [Architecture](architecture.md) |
| Security | [Security](security.md) |
| Operations | [Operations](operations.md) |
| Development | [Development](development.md) |

kegtrack is free: MIT licence, no paid edition. Source and releases: [github.com/selfhostr/kegtrack](https://github.com/selfhostr/kegtrack).

!!! warning "Out of scope for v1"
    Billing, deposits and amounts due, offline operation, a native app, self-service sign-up, a public API, custom statuses, multi-site.
