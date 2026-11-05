# Développement

À la fin de cette page, vous savez construire kegtrack, lancer ses tests et proposer une modification.

Tout se construit en conteneur. `./dev.sh` charge `.env`, monte le dépôt dans l'image `webqr/dev:latest` et se connecte à la base de développement.

## Préparer l'environnement

```bash
git clone https://github.com/selfhostr/kegtrack.git && cd kegtrack
cp .env.exemple .env   # remplir chaque mot de passe
docker build -f docker/Dockerfile.dev -t webqr/dev:latest .
docker network create webqr_dev
docker run -d --name webqr_pg_dev --network webqr_dev \
  -e POSTGRES_DB=webqr -e POSTGRES_PASSWORD=… \
  -e MDP_OWNER=… -e MDP_APP=… -e MDP_EDITEUR=… -e MDP_AUTH=… \
  -v "$PWD/docker/initdb":/docker-entrypoint-initdb.d:ro \
  -v webqr_pgdev:/var/lib/postgresql/data postgres:17-alpine
```

Les mots de passe doivent correspondre à ceux de `.env`.

## Lancer les vérifications

```bash
./dev.sh cargo test --workspace                          # suite complète
./dev.sh cargo test -p web --test acces                  # un seul binaire de test
./dev.sh cargo clippy --workspace --all-targets -- -D warnings
./dev.sh cargo fmt --all --check
./dev.sh cargo sqlx prepare --workspace -- --all-targets # après toute requête modifiée
```

## Tests

Chaque test reçoit une **base PostgreSQL neuve et migrée**, avec les vrais rôles de production. Sans cela, les tests de RLS ne prouveraient rien.

| Binaire | Ce qu'il prouve |
|---|---|
| `isolation` | Six scénarios d'attaque contre la séparation des locataires, sur des tables énumérées depuis le catalogue |
| `acces` | Chaque route avec chaque rôle, plus le CSRF, l'absence de session et la recherche globale limitée à la brasserie |
| `documents` | Atomicité, déduplication, refus précoce, survie du brouillon |
| `coherence` | Une adresse d'un autre client est refusée par le serveur **et** par la base |
| `reconstruction` | Le rejeu donne l'état courant, et une projection corrompue est détectée |
| `proprietes` | Invariants de la machine à états sur des milliers d'historiques générés |

## Intégration continue

Le workflow `.github/workflows/integration.yml` s'exécute à chaque push sur `main` et chaque pull request :

- format, clippy, vérification hors ligne des requêtes SQLx et tests ;
- rejeu des projections sur un jeu de démonstration ;
- décodage QR d'une planche d'étiquettes générée ;
- aucun débordement horizontal sur iPhone 13, Pixel 5 et 320 px (Playwright) ;
- audit des dépendances (`cargo deny`, `cargo audit`) ;
- construction de l'image Docker ;
- exercice de sauvegarde puis de restauration.

## Règles pour les contributeurs

| Si vous… | Alors aussi… |
|---|---|
| ajoutez une route | enregistrez-la avec son rôle dans `web::routes::INVENTAIRE` (sinon un test échoue) |
| modifiez une requête SQLx | relancez `cargo sqlx prepare` (la CI vérifie avec `--check`) et versionnez `.sqlx` |
| ajoutez une table métier | donnez-lui une colonne `tenant_id` et terminez la migration par `select appliquer_rls();` |
| ajoutez une variante de `Statut`, `Role`, `TypeEvenement` ou `TypeDocument` | mettez à jour `TOUS`, `cle_i18n()` et `crates/web/src/i18n/fr.rs` et `en.rs` |
| ajoutez un texte visible | passez par `t!("cle")` (une clé manquante est une erreur de compilation) |
| ajoutez du comportement JavaScript | pas de `onclick` ni de `<script>` en ligne (la CSP les bloque) : utilisez un attribut `data-*` traité par `assets/js/commun.js` |
| ajoutez une ressource statique | mettez-la dans `assets/` et embarquez-la dans `crates/web/src/statique.rs`. Pas de CDN : la CSP est `default-src 'self'`. |
| ajoutez un réglage | lisez-le depuis une variable d'environnement dans `web::Config::depuis_env`, sans jamais coder un secret en dur |

!!! note "Langue"
    Le projet est écrit en français : identifiants, commentaires et messages de commit. Seuls l'infrastructure (Docker, CI) et les noms tiers restent en anglais. Les fichiers destinés à GitHub et ces pages sont en anglais, avec les traductions françaises à côté (`page.fr.md`).

Voir aussi [CONTRIBUTING.md](https://github.com/selfhostr/kegtrack/blob/main/CONTRIBUTING.md).
