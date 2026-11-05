# Install kegtrack

By the end of this page kegtrack runs on your machine and you can sign in to your first brewery.

## Requirements

Docker with the `compose` plugin. Nothing else: no Rust, no Node, no PostgreSQL. Everything builds and runs in containers.

## Create the secrets

Create a `.env` file at the root of the repository (`.env.exemple` is a template). **No default value exists.** The application refuses to start if one of these variables is missing, and it lists all of them at once instead of one per restart.

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

`SECRET_APP` peppers the hashing of passwords and PIN codes, so a stolen database is not enough to attack them offline. **Changing it invalidates every existing password.** See [Rotate the application secret](operations.md#rotate-the-application-secret).

| PostgreSQL role | Use |
|---|---|
| `webqr_owner` | owns the tables, runs the migrations, then is no longer used |
| `webqr_app` | all business queries, **subject to Row Level Security** |
| `webqr_editeur` | publisher back-office only |
| `webqr_auth` | session and password lookup, strictly enumerated rights |

Every variable is listed in [Configuration](configuration.md).

## Start the services

```bash
docker compose up -d --build
docker compose ps        # the four services must be "healthy"
```

On first start PostgreSQL creates the four roles, the application runs its migrations before it opens its port, and the backup service waits for the schema before its first archive.

## Create the first brewery

```bash
docker compose run --rm app provisionner-brasserie \
  --nom "Brasserie du Phare" \
  --slug phare \
  --prefixe-qr PHARE \
  --admin-email contact@brasserie.fr \
  --admin-nom Martin --admin-prenom Claire
```

The command prints a generated strong password. **It is never shown again**: pass it on through a safe channel. To set it yourself, add `--admin-mot-de-passe`. It must be at least 12 characters and mix three character classes.

The QR prefix is **unique across the whole installation**: two breweries cannot issue the same code.

Then open the app on port `6666` (or your domain), sign in, and set your PIN in *My account*. See the [user guide](user-guide.md).

### Add a publisher account (optional)

```bash
docker compose run --rm app creer-editeur --email you@publisher.example
```

It gives access to `/editeur`: create a brewery, suspend it, look at the system state. It is a separate scope with its own cookie and its own accounts table. No brewery account can be promoted into it.

## Load demonstration data (optional)

```bash
docker compose run --rm app semer-demonstration --locataire phare --futs 3000
```

One year of credible history: complete cycles, kegs held by customers, lots whose best-before date is near. Useful for development, for load testing, and for showing the product.

## Print labels

*Administration → QR labels* generates a range, creates the matching kegs and produces a printable sheet for a 3 x 8 layout of 70 x 37 mm labels.

!!! warning "Paper labels do not survive a caustic wash"
    A paper label does not survive a wash at 80 °C. Plan for engraved plates or industrial polyester labels.

## Check that it works

```bash
curl -fsS http://localhost:6666/sante      # the service answers
docker compose exec app webqr --version
docker compose logs -f app
```

`/pret` also checks that the database answers. It is the probe used by the container healthcheck.

!!! warning "The camera needs HTTPS"
    Serving the app on `http://192.168.1.x:6666` makes the browser block the camera. Read [HTTPS and camera](https-camera.md) before you show the product.

To update, see [Update kegtrack](operations.md#update-kegtrack).
