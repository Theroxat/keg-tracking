# Installer kegtrack

À la fin de cette page, kegtrack tourne sur votre machine et vous pouvez ouvrir la session de votre première brasserie.

## Prérequis

Docker avec le greffon `compose`. Rien d'autre : ni Rust, ni Node, ni
PostgreSQL. Tout se construit et tout tourne en conteneurs.

## Créer les secrets

Créez un fichier `.env` à la racine (`.env.exemple` sert de modèle). **Aucune valeur par défaut n'existe** :
l'application refuse de démarrer si l'une de ces variables manque, et elle
les liste toutes d'un coup plutôt qu'une par redémarrage.

```bash
gen() { head -c 32 /dev/urandom | base64 | tr -d '/+=' | head -c 32; }
cat > .env <<EOF
POSTGRES_PASSWORD=$(gen)
MDP_OWNER=$(gen)
MDP_APP=$(gen)
MDP_EDITEUR=$(gen)
MDP_AUTH=$(gen)
SECRET_APP=$(gen)$(gen)
DOMAINE_PUBLIC=qr.exemple.fr
PORT_PUBLIC=6666
FUSEAU_DEFAUT=Europe/Paris
RUST_LOG=info
EOF
chmod 600 .env
```

`SECRET_APP` poivre le hachage des mots de passe et des codes PIN : une base
volée ne suffit pas à les attaquer hors ligne. **Le changer invalide tous
les mots de passe existants** — voir [Changer le secret applicatif](operations.fr.md#changer-le-secret-applicatif).

| Rôle PostgreSQL | À quoi il sert |
|---|---|
| `webqr_owner` | possède les tables, joue les migrations, puis n'est plus utilisé |
| `webqr_app` | toutes les requêtes métier — **soumis à la Row Level Security** |
| `webqr_editeur` | back-office éditeur uniquement |
| `webqr_auth` | résolution de session et mot de passe, droits strictement énumérés |

## Démarrer les services

```bash
docker compose up -d --build
docker compose ps        # les quatre services doivent être « healthy »
```

Au premier démarrage, PostgreSQL crée les quatre rôles, l'application joue
ses migrations avant d'ouvrir son port, et la sauvegarde attend que le
schéma existe avant sa première archive.

## Créer la première brasserie

```bash
docker compose run --rm app provisionner-brasserie \
  --nom "Brasserie du Phare" \
  --slug phare \
  --prefixe-qr PHARE \
  --admin-email contact@brasserie.fr \
  --admin-nom Martin --admin-prenom Claire
```

La commande affiche un mot de passe fort généré. **Il n'est plus affiché
ensuite** : transmettez-le par un canal sûr. Pour l'imposer vous-même,
ajoutez `--admin-mot-de-passe`. Il doit faire au moins 12 caractères et
mélanger trois classes de caractères.

Le préfixe QR est **unique sur toute l'installation** : deux brasseries ne
peuvent pas émettre le même code.

### Ajouter un compte éditeur (facultatif)

```bash
docker compose run --rm app creer-editeur --email vous@editeur.fr
```

Il donne accès à `/editeur` : créer une brasserie, la suspendre, consulter
l'état du système. C'est un périmètre séparé, avec son propre cookie et sa
propre table de comptes — aucun compte de brasserie ne peut y être promu.

## Charger le jeu de démonstration (facultatif)

```bash
docker compose run --rm app semer-demonstration --locataire phare --futs 3000
```

Un an d'historique crédible : cycles complets, fûts immobilisés chez des
clients, lots dont la DLUO approche. Indispensable pour développer, pour
tester la tenue en charge, et pour montrer le produit à un prospect.

Toutes les variables sont listées dans [Configuration](configuration.fr.md).

## Imprimer les étiquettes

`Administration → Étiquettes QR` génère une plage, crée les fûts
correspondants et produit une planche imprimable calée sur un format
3 × 8 étiquettes de 70 × 37 mm.

!!! warning "Une étiquette papier ne survit pas au lavage"
    Une étiquette papier ne survit pas à un lavage à la soude à 80 °C. Prévoyez des plaques gravées ou des étiquettes polyester industrielles.

## Vérifier que tout fonctionne

```bash
curl -fsS http://localhost:6666/sante      # le service répond
docker compose exec app webqr --version
docker compose logs -f app
```

`/pret` vérifie en plus que la base répond ; c'est la sonde utilisée par le
`healthcheck` du conteneur.

!!! warning "La caméra exige HTTPS"
    Servir l'application sur `http://192.168.1.x:6666` fait bloquer la caméra par le navigateur. Lisez [HTTPS et caméra](https-camera.fr.md) avant la démonstration.

Pour mettre à jour, voir [Mettre à jour kegtrack](operations.fr.md#mettre-à-jour-kegtrack).
