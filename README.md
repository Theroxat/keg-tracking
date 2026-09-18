# Suivi des fûts

Traçabilité de fûts de bière consignés par QR code, vendue en SaaS à des
brasseries. Chaque fût porte une étiquette, chaque opération terrain se
fait par scan depuis un téléphone, et **le flux d'événements — jamais
l'enregistrement du fût — fait foi**.

L'application répond à tout moment à : où est ce fût et depuis combien de
temps, que contient-il, combien de fûts propres sont disponibles, lesquels
ne sont pas revenus depuis plus de N jours, et **qui a reçu le lot
2026-017**.

## Démarrer

```bash
cp .env.exemple .env   # puis remplir les secrets — voir docs/installation.md
docker compose up -d --build
docker compose run --rm app provisionner-brasserie \
  --nom "Ma Brasserie" --slug ma-brasserie --prefixe-qr MB \
  --admin-email vous@exemple.fr
```

## Documentation

| Document | Pour qui |
|---|---|
| [docs/installation.md](docs/installation.md) | installer, configurer le HTTPS, régler le blocage caméra |
| [docs/exploitation.md](docs/exploitation.md) | sauvegardes, restauration, audit, assistance |
| [docs/guide-utilisateur.md](docs/guide-utilisateur.md) | le produit, écran par écran |
| [docs/PHASE-0-ARCHITECTURE.md](docs/PHASE-0-ARCHITECTURE.md) | les décisions techniques et leurs raisons |

## Architecture en une page

Rust (Axum + SQLx) sur PostgreSQL, rendu serveur en Maud, htmx pour les
écrans de gestion, et un unique îlot JavaScript autonome pour le scan.
Pas de SPA, pas de chaîne de construction front, pas de CDN.

Quatre crates :

| Crate | Rôle |
|---|---|
| `domaine` | statuts, transitions, projections — sans Axum, sans runtime, sans I/O |
| `persistance` | SQLx, migrations, et la garde qui rend l'isolation impossible à contourner |
| `web` | routes, gabarits, traduction, intergiciels |
| `app` | le binaire : `serve` et les commandes d'exploitation |

Trois garanties structurelles, chacune vérifiée par un test :

1. **Isolation multi-locataire.** Row Level Security PostgreSQL sur chaque
   table métier, avec un rôle applicatif qui n'est pas propriétaire des
   tables. Un `where tenant_id = …` oublié ne retourne **aucune** ligne au
   lieu de toutes.
2. **Le flux d'événements est la vérité.** `evenement_fut` est en ajout
   seul, protégé par un trigger et par les droits. L'état courant est une
   projection recalculable : `reconstruire-projections --verifier` le prouve.
3. **Contrôle d'accès sur chaque route.** Les routes vivent dans des
   groupes porteurs d'un rôle, et un test itère l'inventaire complet avec
   les trois rôles.

## Tests

```bash
./dev.sh cargo test --workspace     # 53 tests, base PostgreSQL neuve par test
./dev.sh cargo clippy --all-targets -- -D warnings
```

| Famille | Ce qu'elle prouve |
|---|---|
| `isolation` | six scénarios d'attaque du cloisonnement, tables énumérées depuis le catalogue |
| `acces` | chaque route × chaque rôle, plus CSRF et absence de session |
| `documents` | atomicité, dédoublonnage, refus anticipé, survie du brouillon |
| `coherence` | adresse d'un autre client refusée par le serveur **et** par la base |
| `reconstruction` | le rejeu redonne l'état courant ; une projection corrompue est détectée |
| `proprietes` | invariants de la machine à états sur des milliers d'histoires engendrées |

## Hors périmètre v1

Facturation, consignes et montants dus, fonctionnement hors ligne,
application native, inscription en libre-service, API publique, statuts
personnalisables, multi-sites. Voir la section A.7 de la spécification.
