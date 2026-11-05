# Sécurité

À la fin de cette page, vous savez ce que kegtrack protège, comment, et ce qu'il ne couvre pas.

Pour signaler une vulnérabilité, utilisez le signalement privé de GitHub, comme décrit dans [SECURITY.md](https://github.com/selfhostr/kegtrack/blob/main/SECURITY.md). Seule la dernière version de `main` est prise en charge.

## Isolation entre locataires

La Row Level Security de PostgreSQL s'applique à toute table qui porte une colonne `tenant_id`. Le rôle applicatif ne possède pas les tables. `Depot::ouvrir(locataire, utilisateur)` est le seul moyen d'obtenir une connexion métier : un `where tenant_id = …` oublié ne renvoie **aucune** ligne au lieu de toutes.

| Rôle PostgreSQL | Usage |
|---|---|
| `webqr_owner` | Migrations uniquement ; sa connexion est fermée ensuite |
| `webqr_app` | Toute la logique métier, soumis à la RLS |
| `webqr_editeur` | Back-office hors locataire |
| `webqr_auth` | Sessions et mots de passe, droits accordés colonne par colonne |

Toute possibilité de lire ou d'écrire les données d'une autre brasserie est traitée comme critique.

## Historique en ajout seul

La table `evenement_fut` est en ajout seul : un déclencheur et des privilèges révoqués interdisent toute modification ou suppression. Toute possibilité d'altérer ou de supprimer des événements est une vulnérabilité.

## Contrôle d'accès

Les routes sont réparties en trois groupes, chacun avec un rôle minimal (Lecture, Production, Administrateur). L'inventaire `routes::INVENTAIRE` est la source unique du test d'accès, qui essaie chaque route avec chaque rôle, ainsi que le CSRF et l'absence de session. Le back-office éditeur est un périmètre séparé, avec son propre cookie et sa propre table de comptes.

## Mots de passe, sessions et PIN

Mots de passe et PIN sont hachés avec Argon2id et poivrés par `SECRET_APP`. Une session dure 30 jours sur un appareil connu, se verrouille après 15 minutes d'inactivité et demande un PIN à six chiffres. Cinq PIN erronés révoquent la session. Lisez [Changer le secret applicatif](operations.fr.md#changer-le-secret-applicatif) avant de modifier le poivre.

## Limitation de débit

Deux niveaux, complémentaires :

- **par adresse IP**, en mémoire : 12 tentatives de connexion par minute, 8 pour le code PIN et le back-office éditeur, 240 scans par minute et par utilisateur. Remis à zéro au redémarrage, ce qui est acceptable pour ce qu'il protège.
- **par compte**, en base : au-delà de 5 échecs, verrouillage exponentiel (1, 2, 4, 8… minutes, plafonné à 30). Celui-là **survit à un redémarrage** : c'est lui qui protège réellement contre le bourrage de mots de passe.

L'adresse réelle du client est lue dans les en-têtes du proxy (`CF-Connecting-IP`, `X-Forwarded-For`) quand `DERRIERE_PROXY` vaut `true`.

## Accès d'assistance

Depuis `/editeur`, le bouton **Assistance** ouvre une session dans la brasserie choisie, sur son premier administrateur actif. Cette session :

- dure une heure au maximum ;
- porte une marque en base (`session.usurpee_par`) ;
- affiche un **bandeau rouge permanent** dans toute l'application ;
- est journalisée à l'ouverture et à la fermeture.

!!! warning "Un pouvoir réel"
    Ne l'utilisez que sur demande du client, et fermez la session dès que le diagnostic est fait.

## Journaux d'audit

- `journal_audit`, par brasserie : connexions, changements de rôle, archivages, imports, validations de documents, modifications de paramètres. Consultable dans *Administration → Journal d'audit*.
- `journal_audit_editeur`, hors brasserie : créations, suspensions, et **chaque ouverture et fermeture d'assistance**.

## Durcissement web

La politique de sécurité du contenu est `default-src 'self'`, sans JavaScript en ligne, et sans CDN. Ne servez l'application que derrière TLS : la caméra l'exige de toute façon (voir [HTTPS et caméra](https-camera.fr.md)), les cookies sont `Secure`, et Caddy envoie `Strict-Transport-Security`.

## Ce qui n'est pas couvert

!!! warning "Limites"
    - Les sauvegardes restent sur la même machine tant que vous ne les copiez pas ailleurs.
    - La limitation de débit par IP est en mémoire et repart de zéro au redémarrage.
    - Ne versionnez jamais `.env` ni aucun vrai mot de passe, nom d'hôte ou IP.
