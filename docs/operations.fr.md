# Exploitation

À la fin de cette page, vous savez mettre à jour, sauvegarder, restaurer, surveiller et dépanner un kegtrack en production.

Toutes les commandes s'exécutent avec `docker compose run --rm app <commande>` ; la liste est dans [Configuration](configuration.fr.md#commandes).

## Mettre à jour kegtrack

```bash
git pull
docker compose up -d --build
```

Les migrations sont jouées automatiquement au démarrage, avant l'ouverture du port. Un verrou consultatif PostgreSQL évite que deux instances ne les jouent en même temps. Faites d'abord une [sauvegarde](#sauvegarder).

## Sauvegarder

Le service `sauvegarde` produit une archive `pg_dump --format=custom` au
démarrage puis chaque nuit à 03h15, avec rotation : 7 quotidiennes,
4 hebdomadaires, 6 mensuelles (configurable par `RETENTION_*`).

Deux garde-fous, parce que les deux pannes silencieuses classiques sont
d'archiver une base pas encore migrée et d'archiver une base vide sans s'en
apercevoir :

1. la première sauvegarde **attend** que le schéma existe ;
2. chaque archive est relue (`pg_restore --list`) et rejetée si elle ne
   contient pas les données de `evenement_fut`.

```bash
docker compose exec sauvegarde ls -lh /sauvegardes/quotidien
docker compose logs sauvegarde --tail 20
```

Les archives vivent dans le volume `sauvegardes`, sur la même machine. Copiez-les ailleurs, et gardez votre `.env` avec elles.

## Restaurer

Une sauvegarde jamais restaurée n'est pas une sauvegarde. Celle-ci est
rejouée par l'intégration continue à chaque modification, et voici la
version manuelle.

**Répétition sans risque**, dans une base jetable :

```bash
DUMP=$(docker compose exec -T sauvegarde sh -c 'ls -1t /sauvegardes/quotidien/*.dump | head -1' | tr -d '\r')

docker compose exec -T postgres psql -U postgres -d postgres \
  -c "drop database if exists webqr_restaure with (force);" \
  -c "create database webqr_restaure;"

docker compose exec -T sauvegarde \
  pg_restore -h postgres -U postgres --dbname=webqr_restaure \
             --no-owner --no-privileges "$DUMP"

docker compose exec -T postgres psql -U postgres -d webqr_restaure -tAc \
  "select (select count(*) from fut) || ' fûts, ' ||
          (select count(*) from evenement_fut) || ' événements'"
```

Comparez avec la base vivante. Puis `drop database webqr_restaure`.

**Restauration réelle**, après incident :

```bash
docker compose stop app caddy
docker compose exec -T postgres psql -U postgres -d postgres \
  -c "drop database webqr with (force);" -c "create database webqr owner webqr_owner;"
docker compose exec -T sauvegarde pg_restore -h postgres -U postgres --dbname=webqr "$DUMP"
docker compose start app caddy
docker compose run --rm app reconstruire-projections --verifier
```

La dernière ligne n'est pas décorative : elle prouve que la base restaurée
est cohérente, en rejouant tout le flux d'événements et en le comparant à
l'état courant.

## Vérifier les projections

L'état d'un fût affiché dans les listes est un **cache**. La vérité est la
suite de ses événements, en ajout seul.

```bash
# Vérifie sans rien écrire ; sort en erreur s'il existe un écart.
docker compose run --rm app reconstruire-projections --verifier

# Recalcule et corrige.
docker compose run --rm app reconstruire-projections --locataire phare
```

À lancer après une restauration, après une correction de bug de projection,
ou simplement par acquit de conscience. Le rejeu est idempotent.

## Exporter ou supprimer une brasserie

```bash
docker compose run --rm app exporter-locataire --locataire phare --sortie phare.json
docker compose run --rm app supprimer-locataire --locataire phare --confirmer
```

L'export contient toutes les tables du locataire au format JSON ouvert.
La suppression est définitive et emporte tout par cascade ; elle est
journalisée dans le journal éditeur.

## Changer le secret applicatif

`SECRET_APP` poivre Argon2id. **Le changer invalide tous les mots de passe
et tous les codes PIN.** Si vous devez le faire :

1. prévenez les utilisateurs ;
2. changez la variable et redémarrez ;
3. réinitialisez chaque mot de passe depuis
   `Administration → Utilisateurs → Réinitialiser`.

Il n'y a pas de migration en douceur : c'est le prix du poivre, et il est
volontaire.

## Surveiller

Journalisation structurée sur la sortie standard, niveau réglé par
`RUST_LOG` (par exemple `info,web=debug,sqlx=warn`).

| Point | Usage |
|---|---|
| `GET /sante` | vivacité : le processus répond |
| `GET /pret` | disponibilité : la base répond aussi (utilisé par le `healthcheck`) |

Les deux journaux d'audit sont décrits dans [Sécurité](security.fr.md#journaux-daudit).

## Dépanner

| Symptôme | À vérifier |
|---|---|
| L'application ne démarre pas | `docker compose logs app` : elle liste d'un coup toutes les variables manquantes |
| La caméra n'affiche rien | La page n'est pas servie en HTTPS : voir [HTTPS et caméra](https-camera.fr.md) |
| Plus personne ne peut se connecter après un changement | `SECRET_APP` a changé : réinitialisez les mots de passe |
| Les listes contredisent la chronologie | Lancez `reconstruire-projections --verifier` |

## Passer à l'échelle

!!! warning "Instance unique"
    Le déploiement v1 est mono-instance. La limitation de débit par IP est en mémoire.

Pour passer à plusieurs instances
d'application derrière le même proxy, deux points à traiter :

- la limitation de débit par IP est en mémoire : la déporter en base ou
  dans un cache partagé ;
- rien d'autre. Les sessions, les brouillons de capture et les compteurs de
  référence vivent déjà en base.

PostgreSQL reste l'unique point d'état. Les index et la pagination côté
serveur sont dimensionnés pour un parc de plusieurs milliers de fûts et
plusieurs centaines de milliers d'événements.
