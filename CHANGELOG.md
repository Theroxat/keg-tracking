# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.0.0] - 2026-10-17

First release: keg traceability for breweries, from the workshop floor to the office.

### Added

- Four-crate Rust workspace (`domaine`, `persistance`, `web`, `app`) on Axum, SQLx and PostgreSQL, with server-side rendering (Maud), htmx and a single standalone JavaScript scan island.
- Multi-tenant isolation through PostgreSQL Row Level Security and four database roles; a forgotten `where` returns no rows instead of all of them.
- Append-only event stream as the source of truth; keg state is a rebuildable projection (`reconstruire-projections --verifier`).
- Role-based access (read, production, administrator) enforced on every route and tested against the full route inventory.
- Keg fleet with filters and export, keg page with timeline, customers and addresses.
- Single-keg scan (BarcodeDetector, zxing-wasm fallback, handheld scanner) and batch operations with persisted drafts and transactional validation.
- Delivery notes, product batches with recall, printable QR labels, import of existing data, settings and audit log.
- Publisher back-office with logged assistance, and operations commands (provision, export, delete tenant, seed demo).
- Global search (kegs, customers, batches) with the `/` shortcut, and a batch page with editing and beer / best-before filters.
- Quick actions on the desktop dashboard, filterable and paginated audit log, deep links that prefill operations (deliver to a customer, fill from a batch).
- CSV import template, encrypted label preview and empty states with useful next steps.
- Installation, operations and user documentation.

### Changed

- "Atelier clair" visual redesign: warm light background, borderless soft-shadow surfaces, large radii, teal accent, 48 px touch targets, 16 px fields, pill status badges and a floating mobile bar.
- Simplified mobile workflow: role-based bottom bar with a large Scan button, collapsed manual entry, single "Finish" button for capture, operation presets, keg page centered on the next action, touch PIN pad.
- Lighter keg page (collapsed editing, latest 10 events, collapsed full history) and refined desktop layout.
- Simplified routes and scan island (shared helpers, extracted state and timeline cards, dead code removed).
- Capture screen clears the bottom of the screen; batch numbers are monospace.

### Fixed

- Mobile layout: two-column pages no longer cover the content on narrow screens.
- Horizontal overflow of the keg page on mobile.
- Confirmations (customer archiving, label generation) blocked by the CSP; label sheet "Print" button now works.
- Location sort order, search results bullets, read-only field styling and button variant specificity.

### Security

- Strict Content Security Policy: no `'unsafe-inline'` in `script-src`; confirmations, printing and htmx redirects handled by a static script.
- Tenant non-traversal test for global search.
