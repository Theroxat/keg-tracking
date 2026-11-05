# Configuration

À la fin de cette page, vous connaissez chaque variable, commande, port, volume et point de contrôle.

La configuration passe uniquement par des variables d'environnement, dans `.env`. Si des variables manquent, l'application refuse de démarrer et les liste **toutes** d'un coup.

## Variables d'environnement

À définir dans `.env`, transmises par `compose.yaml` :

| Variable | Rôle | Défaut |
|---|---|---|
| `POSTGRES_PASSWORD` | Superutilisateur PostgreSQL (initialisation, sauvegardes) | aucun, obligatoire |
| `MDP_OWNER`, `MDP_APP`, `MDP_EDITEUR`, `MDP_AUTH` | Mots de passe des quatre rôles PostgreSQL | aucun, obligatoire |
| `SECRET_APP` | Poivre Argon2id, au moins 32 caractères. **Le changer invalide tous les mots de passe et PIN.** | aucun, obligatoire |
| `DOMAINE_PUBLIC` | Domaine servi, par exemple `qr.exemple.fr` | aucun, obligatoire |
| `PORT_PUBLIC` | Port exposé par Caddy | `6666` |
| `FUSEAU_DEFAUT` | Fuseau horaire | `Europe/Paris` |
| `RUST_LOG` | Niveau de journalisation, par exemple `info,web=debug,sqlx=warn` | `info` |
| `RETENTION_QUOTIDIENNE` | Sauvegardes quotidiennes conservées | `7` |
| `RETENTION_HEBDOMADAIRE` | Sauvegardes hebdomadaires conservées | `4` |
| `RETENTION_MENSUELLE` | Sauvegardes mensuelles conservées | `6` |

Posées par `compose.yaml` pour le service `app`, lues par l'application :

| Variable | Rôle | Défaut |
|---|---|---|
| `DATABASE_URL_OWNER`, `_APP`, `_EDITEUR`, `_AUTH` | Une URL de connexion par rôle PostgreSQL, construite avec les mots de passe ci-dessus | aucun, obligatoire |
| `ADRESSE_ECOUTE` | Adresse d'écoute de l'application | `0.0.0.0:8080` |
| `DERRIERE_PROXY` | Lire l'adresse réelle du client dans les en-têtes transmis par le proxy | `true` |
| `COOKIES_SECURISES` | Le navigateur voit du HTTPS même si l'application reçoit du HTTP (derrière un tunnel). Les cookies restent `Secure`. | `true` |
| `CONNEXIONS_MAX` | Nombre maximal de connexions à la base | `16` |

## Commandes

Toutes s'exécutent avec `docker compose run --rm app <commande>`.

| Commande | Effet |
|---|---|
| `serve` | Démarre le serveur. C'est la commande par défaut du conteneur. |
| `provisionner-brasserie` | Crée une brasserie, son premier administrateur et ses emplacements de base |
| `creer-editeur` | Crée un compte du back-office éditeur |
| `semer-demonstration` | Injecte un parc de démonstration avec un an d'historique |
| `reconstruire-projections [--verifier]` | Rejoue le flux d'événements. Avec `--verifier`, détecte les écarts sans rien écrire. |
| `exporter-locataire` | Exporte toutes les données d'une brasserie en JSON |
| `supprimer-locataire` | Supprime définitivement une brasserie |

Les cas d'usage sont dans [Installer kegtrack](installation.fr.md) et [Exploitation](operations.fr.md).

## Services, ports et volumes

| Service | Image | Port | Rôle |
|---|---|---|---|
| `caddy` | `caddy:2-alpine` | `PORT_PUBLIC` (`6666` par défaut) | Proxy inverse, compression, en-têtes |
| `app` | construite depuis `docker/Dockerfile` | `8080` (interne) | Le binaire `webqr` |
| `postgres` | `postgres:17-alpine` | `5432` (interne) | Le seul état |
| `sauvegarde` | `postgres:17-alpine` | aucun | `pg_dump` nocturne avec rotation |

| Volume | Contenu |
|---|---|
| `pg_data` | Les données PostgreSQL |
| `sauvegardes` | Les archives de sauvegarde |
| `caddy_data` | Les certificats de Caddy |
| `caddy_config` | La configuration de Caddy |

## Points de contrôle

| Point | Sens |
|---|---|
| `GET /sante` | Vivacité : le processus répond. Répondu par Caddy. |
| `GET /pret` | Disponibilité : la base répond aussi. Utilisé par le `healthcheck` du conteneur. |
