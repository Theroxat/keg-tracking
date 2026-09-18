# Installation

## Ce qu'il faut sur la machine

Docker avec le greffon `compose`. Rien d'autre : ni Rust, ni Node, ni
PostgreSQL. Tout se construit et tout tourne en conteneurs.

## 1. Les secrets

Créez un fichier `.env` à la racine. **Aucune valeur par défaut n'existe** :
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
les mots de passe existants** — voir `exploitation.md`.

| Rôle PostgreSQL | À quoi il sert |
|---|---|
| `webqr_owner` | possède les tables, joue les migrations, puis n'est plus utilisé |
| `webqr_app` | toutes les requêtes métier — **soumis à la Row Level Security** |
| `webqr_editeur` | back-office éditeur uniquement |
| `webqr_auth` | résolution de session et mot de passe, droits strictement énumérés |

## 2. Démarrage

```bash
docker compose up -d --build
docker compose ps        # les quatre services doivent être « healthy »
```

Au premier démarrage, PostgreSQL crée les quatre rôles, l'application joue
ses migrations avant d'ouvrir son port, et la sauvegarde attend que le
schéma existe avant sa première archive.

## 3. Première brasserie

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

### Compte éditeur (facultatif)

```bash
docker compose run --rm app creer-editeur --email vous@editeur.fr
```

Il donne accès à `/editeur` : créer une brasserie, la suspendre, consulter
l'état du système. C'est un périmètre séparé, avec son propre cookie et sa
propre table de comptes — aucun compte de brasserie ne peut y être promu.

## 4. Jeu de démonstration

```bash
docker compose run --rm app semer-demonstration --locataire phare --futs 3000
```

Un an d'historique crédible : cycles complets, fûts immobilisés chez des
clients, lots dont la DLUO approche. Indispensable pour développer, pour
tester la tenue en charge, et pour montrer le produit à un prospect.

---

## Le blocage caméra : à régler avant la démonstration

`getUserMedia` **exige un contexte sécurisé**. Servir l'application sur
`http://192.168.1.x:6666` bloque la caméra : le navigateur refuse, sans
message clair, et le produit ne fonctionne plus. Ce n'est pas un détail de
confort.

Trois situations, trois réponses.

### a. Tunnel Cloudflare — la configuration de référence

C'est le montage retenu ici. Le tunnel publie `https://qr.exemple.fr` vers
`http://<cette machine>:6666`, et c'est lui qui fournit le certificat.
Caddy sert donc en clair derrière lui.

```
Navigateur ──https──> Cloudflare ──tunnel──> 192.168.x.x:6666 ──> Caddy ──> app
```

Rien à configurer côté application, sinon `DOMAINE_PUBLIC` et
`DERRIERE_PROXY=true` (valeur par défaut) pour que l'adresse réelle du
client soit lue dans `CF-Connecting-IP` — c'est elle qui alimente la
limitation de débit.

Côté Cloudflare, le service public doit pointer sur `http://<ip>:6666`.

### b. Nom de domaine pointé directement sur la machine

Modifiez `docker/Caddyfile` :

```caddyfile
{
    servers { trusted_proxies static private_ranges }
}

{$DOMAINE_PUBLIC} {
    encode zstd gzip
    reverse_proxy app:8080
}
```

et publiez les ports 80 et 443 au lieu de 6666. Caddy obtient et renouvelle
le certificat tout seul. Conservez le volume `caddy_data` : il contient les
certificats.

### c. Test sur un vrai téléphone, en réseau local

C'est le cas qui piège. `https://localhost` est un contexte sécurisé, mais
`https://192.168.1.20` ne l'est pas sans certificat reconnu.

1. **Recommandé** : un tunnel — `cloudflared tunnel --url http://localhost:6666`
   ou `tailscale funnel 6666`. URL publique, certificat valide, aucun port
   à ouvrir, et **cela fonctionne sur iOS** sans manipuler de profil.
2. **Repli Android** : `mkcert` pour émettre un certificat local, puis
   installer l'autorité racine sur le téléphone.

Sans contexte sécurisé, l'écran de scan reste utilisable : la **douchette**
et la **saisie manuelle** fonctionnent toujours, et l'application affiche
un message explicite au lieu d'un viseur noir.

---

## Étiquettes

`Administration → Étiquettes QR` génère une plage, crée les fûts
correspondants et produit une planche imprimable calée sur un format
3 × 8 étiquettes de 70 × 37 mm.

> Une étiquette papier ne survit pas à un lavage à la soude à 80 °C.
> Prévoyez des plaques gravées ou des étiquettes polyester industrielles.

## Mise à jour

```bash
git pull
docker compose up -d --build
```

Les migrations sont jouées automatiquement au démarrage, avant l'ouverture
du port. Un verrou consultatif PostgreSQL évite que deux instances ne les
jouent en même temps.

## Vérifier que tout va bien

```bash
curl -fsS http://localhost:6666/sante      # le service répond
docker compose exec app webqr --version
docker compose logs -f app
```

`/pret` vérifie en plus que la base répond ; c'est la sonde utilisée par le
`healthcheck` du conteneur.
