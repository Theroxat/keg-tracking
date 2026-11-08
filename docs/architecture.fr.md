# Architecture

À la fin de cette page, vous savez comment kegtrack est construit et pourquoi.

**Rust** (Axum et SQLx) sur **PostgreSQL 17**, rendu côté serveur avec **Maud**, **htmx** pour les écrans de gestion, et trois fichiers JavaScript autonomes : `scan/scan.js` (scan), `pin.js` (pavé PIN) et `commun.js` (confirmations, impression, raccourci `/`, redirection htmx sur 401/403). La CSP n'autorise aucun JavaScript en ligne. Pas de SPA, pas de chaîne de build front, pas de CDN.

## Crates

| Crate | Rôle |
|---|---|
| `domaine` | Statuts, transitions, projections. Ni Axum, ni runtime, ni E/S. |
| `persistance` | SQLx, migrations embarquées, `Depot` et la garde d'isolation |
| `web` | Routes, gabarits Maud, i18n, middleware |
| `app` | Le binaire `webqr` : `serve` et les commandes d'exploitation |

Les dépendances ne vont que dans un sens :

```
app --> web --> persistance --> domaine
```

## Déploiement

```
Navigateur --https--> tunnel TLS / proxy --> Caddy :6666 --> app (webqr serve) --> PostgreSQL 17
                                                             service de sauvegarde --> PostgreSQL 17
```

Quatre services : `caddy`, `app`, `postgres`, `sauvegarde`. Voir [Configuration](configuration.fr.md#services-ports-et-volumes).

## Trois garanties structurelles

Elles sont la raison d'être de l'architecture, et chacune a son propre test.

1. **Isolation multi-locataire.** Row Level Security PostgreSQL avec quatre rôles. Voir [Sécurité](security.fr.md#isolation-entre-locataires).
2. **Le flux d'événements est la source de vérité.** La table `evenement_fut` est en ajout seul. La table `fut` n'est qu'une projection, recalculée par la même fonction pure (`domaine::projection::appliquer`) en temps réel et lors d'un rejeu complet : les deux chemins ne peuvent pas diverger. Seul `persistance::evenement` écrit dans `fut`.
3. **Contrôle d'accès sur chaque route.** Voir [Sécurité](security.fr.md#contrôle-daccès).
