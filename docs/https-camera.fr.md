# Faire fonctionner la caméra

À la fin de cette page, l'écran de scan peut utiliser la caméra du téléphone.

`getUserMedia` **exige un contexte sécurisé**. Servir l'application sur
`http://192.168.1.x:6666` bloque la caméra : le navigateur refuse, sans
message clair, et le produit ne fonctionne plus. Ce n'est pas un détail de
confort.

!!! warning "Sans contexte sécurisé"
    L'écran de scan reste utilisable : la **douchette** et la **saisie manuelle** fonctionnent toujours, et l'application affiche un message explicite au lieu d'un viseur noir.

Trois situations, trois réponses.

## Publier derrière un tunnel Cloudflare

C'est le montage retenu ici. Le tunnel publie `https://qr.exemple.fr` vers
`http://<cette machine>:6666`, et c'est lui qui fournit le certificat.
Caddy sert donc en clair derrière lui.

```
Navigateur ──https──> Cloudflare ──tunnel──> 192.168.x.x:6666 ──> Caddy ──> app
```

Rien à configurer côté application, sinon `DOMAINE_PUBLIC` et
`DERRIERE_PROXY=true` (valeur par défaut) pour que l'adresse réelle du
client soit lue dans `CF-Connecting-IP` — c'est elle qui alimente la
[limitation de débit](security.fr.md#limitation-de-débit).

Côté Cloudflare, le service public doit pointer sur `http://<ip>:6666`.

## Pointer un nom de domaine directement sur la machine

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

## Tester sur un vrai téléphone, en réseau local

C'est le cas qui piège. `https://localhost` est un contexte sécurisé, mais
`https://192.168.1.20` ne l'est pas sans certificat reconnu.

1. **Recommandé** : un tunnel — `cloudflared tunnel --url http://localhost:6666`
   ou `tailscale funnel 6666`. URL publique, certificat valide, aucun port
   à ouvrir, et **cela fonctionne sur iOS** sans manipuler de profil.
2. **Repli Android** : `mkcert` pour émettre un certificat local, puis
   installer l'autorité racine sur le téléphone.
