# Suivi des fûts

Traçabilité de fûts de bière consignés par QR code, proposée en SaaS aux
brasseries. Chaque fût porte une étiquette, et chaque opération terrain se
fait en scannant cette étiquette depuis un téléphone.

C'est l'historique des opérations qui fait foi, pas la fiche du fût.
L'application répond à tout moment aux questions suivantes :

- **Où est ce fût**, et depuis combien de temps ?
- **Que contient-il ?** Quelle bière, quel lot, quelle cuve ?
- **Combien de fûts propres** sont disponibles pour un remplissage ?
- **Lesquels ne sont pas revenus** depuis plus de N jours ?
- **Qui a reçu le lot 2026-017 ?** C'est le rappel produit, en un clic.

---

## Sommaire

- [Fonctionnalités](#fonctionnalités)
- [Démarrage rapide](#démarrage-rapide)
- [Configuration](#configuration)
- [Commandes d'exploitation](#commandes-dexploitation)
- [Développement](#développement)
- [Architecture](#architecture)
- [Tests et intégration continue](#tests-et-intégration-continue)
- [Documentation](#documentation)
- [Hors périmètre v1](#hors-périmètre-v1)

---

## Fonctionnalités

### Cycle de vie d'un fût

```
(mise en service) → Reçu → Sale → Propre → Rempli → Livré ─┐
                      ▲                                     │
                      └──────────── retour brasserie ───────┘

         + Hors service (réparable)   + Perdu (l'historique est conservé)
```

Une transition incohérente est refusée, avec un message clair. Certaines
transitions demandent des informations : passer à **Rempli** demande une
bière, un lot et une cuve, et passer à **Livré** demande un client et une
adresse. Un administrateur peut forcer une transition hors du modèle, à
condition de la justifier. Elle reste alors marquée en rouge dans la
chronologie du fût.

### Sur le terrain

| Écran | Ce qu'il fait |
|---|---|
| **Scanner** | Accepte une douchette Bluetooth (vue comme un clavier), la caméra arrière avec torche, ou la saisie manuelle. Chaque lecture donne trois retours à la fois : couleur plein écran, son et vibration. |
| **Opération par lot** | On configure une fois, puis on scanne des dizaines de fûts à la suite. Un fût incompatible avec l'opération est refusé au moment du scan, pendant qu'on l'a en main. La capture est enregistrée côté serveur et survit à un rafraîchissement ou à une batterie à plat. La validation est transactionnelle : tout passe, ou rien n'est écrit. |
| **Bon de livraison** | Généré dès la validation d'une livraison, imprimable, avec deux emplacements de signature. |

### Au bureau

| Écran | Ce qu'il fait |
|---|---|
| **Tableau de bord** | Différent selon le rôle. Le bureau voit les compteurs par statut, les fûts non revenus, les alertes DLUO et l'activité récente. L'atelier voit deux grands boutons et les deux chiffres qui comptent. |
| **Parc** | Recherche plein texte, filtres combinables, tri par colonne. L'export CSV ou Excel reprend exactement les filtres actifs. |
| **Fiche d'un fût** | État courant, chronologie complète, et uniquement les actions réellement autorisées depuis son statut. |
| **Clients** | Fûts détenus par chaque client, avec leur ancienneté, en rouge au-delà du seuil. Un client qui détient des fûts ne peut pas être archivé, et un client n'est jamais supprimé. |
| **Lots et rappel produit** | Tous les clients concernés par un lot, avec leurs coordonnées, les quantités et les codes QR exacts. Exportable et imprimable. |
| **Administration** | Référentiels (bières, emplacements, cuves), utilisateurs, planches d'étiquettes QR (3 × 8, 70 × 37 mm), import de reprise depuis un tableur (entièrement validé avant toute écriture), paramètres et journal d'audit. |

### Rôles

| Rôle | Droits |
|---|---|
| **Lecture** | Consulter et exporter. Aucune écriture. |
| **Production** | Scanner, créer et valider des opérations, modifier des fûts. |
| **Administrateur** | Tout dans sa brasserie : utilisateurs, référentiels, paramètres, transitions forcées. |
| *Éditeur* | Back-office séparé (`/editeur`), hors de toute brasserie : créer, suspendre, superviser. |

La session dure 30 jours sur un appareil connu. Après 15 minutes
d'inactivité, elle se verrouille et redemande un **code PIN à six
chiffres**, pour rester utilisable avec des gants.

### Sur téléphone

Sous 720 px, l'interface change de forme : les listes deviennent des cartes,
les actions passent avant la chronologie, les cibles tactiles font au moins
44 px, et une barre de navigation basse reprend les écrans de l'atelier.

---

## Démarrage rapide

**Prérequis :** Docker avec le greffon `compose`. Il n'y a besoin ni de
Rust, ni de Node, ni de PostgreSQL sur la machine.

```bash
# 1. Secrets : aucune valeur par défaut n'existe
gen() { head -c 32 /dev/urandom | base64 | tr -d '/+=' | head -c 32; }
cp .env.exemple .env
# remplir .env avec gen() pour chaque mot de passe, SECRET_APP=$(gen)$(gen)
chmod 600 .env

# 2. Lancement : Caddy, application, PostgreSQL et sauvegardes
docker compose up -d --build
docker compose ps                     # les services doivent être « healthy »

# 3. Première brasserie (le mot de passe administrateur s'affiche une seule fois)
docker compose run --rm app provisionner-brasserie \
  --nom "Brasserie du Phare" --slug phare --prefixe-qr PHARE \
  --admin-email contact@brasserie.fr --admin-nom Martin --admin-prenom Claire

# 4. Facultatif : un an d'historique de démonstration
docker compose run --rm app semer-demonstration --locataire phare --futs 3000
```

Pour vérifier le service : `curl -fsS http://localhost:6666/sante`.

> ⚠️ **La caméra exige HTTPS.** Un navigateur bloque `getUserMedia` sur
> `http://192.168.x.x`, sans message clair. Il faut un tunnel Cloudflare,
> un nom de domaine avec certificat, ou la procédure de test en réseau
> local décrite dans [docs/installation.md](docs/installation.md#le-blocage-caméra--à-régler-avant-la-démonstration).

> 🏷️ **Une étiquette papier ne survit pas** à un lavage à la soude à 80 °C.
> Prévoyez des plaques gravées ou des étiquettes polyester industrielles.

**Mise à jour :** `git pull && docker compose up -d --build`. Les migrations
sont jouées automatiquement au démarrage.

---

## Configuration

La configuration passe uniquement par des variables d'environnement, dans
`.env`. Si des variables manquent, l'application refuse de démarrer et les
liste **toutes** d'un coup.

| Variable | Rôle |
|---|---|
| `POSTGRES_PASSWORD` | superutilisateur PostgreSQL (initialisation, sauvegardes) |
| `MDP_OWNER`, `MDP_APP`, `MDP_EDITEUR`, `MDP_AUTH` | mots de passe des quatre rôles PostgreSQL (voir [Architecture](#architecture)) |
| `SECRET_APP` | poivre Argon2id, au moins 32 caractères. **Le changer invalide tous les mots de passe et codes PIN.** |
| `DOMAINE_PUBLIC` | domaine servi, par exemple `qr.exemple.fr` |
| `PORT_PUBLIC` | port exposé par Caddy (`6666` par défaut) |
| `FUSEAU_DEFAUT` | fuseau horaire (`Europe/Paris` par défaut) |
| `RUST_LOG` | niveau de journalisation (`info` par défaut) |
| `RETENTION_QUOTIDIENNE` / `_HEBDOMADAIRE` / `_MENSUELLE` | nombre de sauvegardes conservées (7 / 4 / 6) |

---

## Commandes d'exploitation

Elles s'exécutent toutes avec `docker compose run --rm app <commande>` :

| Commande | Effet |
|---|---|
| `provisionner-brasserie` | crée une brasserie et son premier administrateur |
| `creer-editeur` | crée un compte du back-office éditeur |
| `semer-demonstration` | injecte un jeu de démonstration crédible |
| `reconstruire-projections [--verifier]` | rejoue le flux d'événements ; avec `--verifier`, détecte les écarts sans rien écrire |
| `exporter-locataire` | exporte toutes les données d'une brasserie |
| `supprimer-locataire` | supprime définitivement une brasserie |

Les sauvegardes, la procédure de restauration, la rotation du secret et
l'accès d'assistance sont décrits dans
[docs/exploitation.md](docs/exploitation.md).

---

## Développement

Tout se compile dans un conteneur. `./dev.sh` charge `.env`, monte le dépôt
dans l'image `webqr/dev:latest` et se branche sur la base de développement.

**Préparation, une seule fois :**

```bash
docker build -f docker/Dockerfile.dev -t webqr/dev:latest .
docker network create webqr_dev
docker run -d --name webqr_pg_dev --network webqr_dev \
  -e POSTGRES_DB=webqr -e POSTGRES_PASSWORD=… \
  -e MDP_OWNER=… -e MDP_APP=… -e MDP_EDITEUR=… -e MDP_AUTH=… \
  -v "$PWD/docker/initdb":/docker-entrypoint-initdb.d:ro \
  -v webqr_pgdev:/var/lib/postgresql/data postgres:17-alpine
```

Les mots de passe doivent être ceux du `.env`.

**Au quotidien :**

```bash
./dev.sh cargo test --workspace                          # suite complète
./dev.sh cargo test -p web --test acces                  # un seul binaire de test
./dev.sh cargo clippy --workspace --all-targets -- -D warnings
./dev.sh cargo fmt --all --check
./dev.sh cargo sqlx prepare --workspace -- --all-targets # après toute requête modifiée
```

### Règles à respecter

| Si vous… | Il faut aussi… |
|---|---|
| ajoutez une route | l'inscrire avec son rôle dans `web::routes::INVENTAIRE` (sinon un test échoue) |
| modifiez une requête SQLx | rejouer `cargo sqlx prepare` (la CI vérifie avec `--check`) |
| ajoutez une table métier | lui donner une colonne `tenant_id` et terminer la migration par `select appliquer_rls();` |
| ajoutez une variante de `Statut`, `Role`, `TypeEvenement` ou `TypeDocument` | mettre à jour `TOUS`, `cle_i18n()` et `crates/web/src/i18n/fr.rs` |
| ajoutez un texte visible | passer par `t!("cle")` (une clé absente est une erreur de compilation) |
| ajoutez une ressource statique | la placer dans `assets/` et l'embarquer dans `crates/web/src/statique.rs`. Pas de CDN : la CSP est `default-src 'self'`. |

---

## Architecture

**Rust** (Axum + SQLx) sur **PostgreSQL 17**, rendu serveur en **Maud**,
**htmx** pour les écrans de gestion, et un unique fichier JavaScript
autonome pour le scan. Il n'y a ni SPA, ni chaîne de construction front,
ni CDN.

```
Navigateur ──https──> (tunnel / proxy TLS) ──> Caddy :6666 ──> app (webqr serve) ──> PostgreSQL
                                                                                       ▲
                                                                       sauvegarde ─────┘
```

### Quatre crates, dépendances strictement descendantes

```
app  →  web  →  persistance  →  domaine
```

| Crate | Rôle |
|---|---|
| `domaine` | statuts, transitions, projections. Pas d'Axum, pas de runtime, pas d'I/O. |
| `persistance` | SQLx, migrations embarquées, `Depot` et la garde d'isolation |
| `web` | routes, gabarits Maud, i18n, intergiciels |
| `app` | le binaire `webqr` : `serve` et les commandes d'exploitation |

### Trois garanties structurelles

Elles sont la raison d'être de l'architecture, et chacune a son test.

1. **Isolation multi-locataire.** La Row Level Security PostgreSQL
   s'applique à toute table qui porte une colonne `tenant_id`. Le rôle
   applicatif n'est pas propriétaire des tables. `Depot::ouvrir(locataire,
   utilisateur)` est le seul moyen d'obtenir une connexion métier. Un
   `where tenant_id = …` oublié renvoie donc **aucune** ligne, au lieu de
   toutes.

   | Rôle PostgreSQL | Usage |
   |---|---|
   | `webqr_owner` | migrations uniquement, sa connexion est refermée ensuite |
   | `webqr_app` | tout le métier, soumis à la RLS |
   | `webqr_editeur` | back-office hors locataire |
   | `webqr_auth` | sessions et mots de passe, droits accordés colonne par colonne |

2. **Le flux d'événements fait foi.** La table `evenement_fut` est en ajout
   seul : un trigger et les droits interdisent toute modification ou
   suppression. La table `fut` n'est qu'une projection, recalculée par la
   même fonction pure (`domaine::projection::appliquer`) en temps réel et
   lors d'un rejeu complet. Les deux chemins ne peuvent donc pas diverger.

3. **Contrôle d'accès sur chaque route.** Les routes sont rangées dans trois
   groupes, chacun associé à un rôle minimal. L'inventaire
   `routes::INVENTAIRE` est la source unique du test d'accès, qui essaie
   chaque route avec chaque rôle.

---

## Tests et intégration continue

Chaque test reçoit une **base PostgreSQL neuve**, migrée, avec les rôles
réels de production. Sans cela, les tests de RLS ne prouveraient rien.

| Binaire | Ce qu'il prouve |
|---|---|
| `isolation` | six scénarios d'attaque du cloisonnement, sur des tables énumérées depuis le catalogue |
| `acces` | chaque route avec chaque rôle, plus le CSRF et l'absence de session |
| `documents` | atomicité, dédoublonnage, refus anticipé, survie du brouillon |
| `coherence` | une adresse d'un autre client est refusée par le serveur **et** par la base |
| `reconstruction` | le rejeu redonne l'état courant, et une projection corrompue est détectée |
| `proprietes` | invariants de la machine à états, sur des milliers d'historiques générés |

La CI GitHub Actions (`.github/workflows/integration.yml`) joue à chaque
push sur `main` et à chaque pull request :

- format, clippy, vérification des requêtes SQLx hors ligne et tests ;
- rejeu des projections sur un jeu de démonstration ;
- décodage des QR d'une planche d'étiquettes générée ;
- absence de débordement horizontal sur iPhone 13, Pixel 5 et 320 px
  (Playwright) ;
- audit des dépendances (`cargo deny`, `cargo audit`) ;
- construction de l'image Docker ;
- exercice de sauvegarde puis restauration.

---

## Documentation

| Document | Pour qui |
|---|---|
| [docs/installation.md](docs/installation.md) | l'installateur : secrets, démarrage, HTTPS et caméra, étiquettes |
| [docs/exploitation.md](docs/exploitation.md) | l'exploitant : sauvegardes, restauration, audit, assistance, limitation de débit |
| [docs/guide-utilisateur.md](docs/guide-utilisateur.md) | l'utilisateur : le produit, écran par écran |
| [docs/PHASE-0-ARCHITECTURE.md](docs/PHASE-0-ARCHITECTURE.md) | les décisions techniques et leurs raisons. Ce document précède le code : en cas de contradiction, le code fait foi. |

---

## Hors périmètre v1

Facturation, consignes et montants dus, fonctionnement hors ligne,
application native, inscription en libre-service, API publique, statuts
personnalisables, multi-sites.
