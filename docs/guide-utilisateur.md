# Guide d'utilisation

## Les trois rôles

| Rôle | Ce qu'il peut faire |
|---|---|
| **Lecture** | Consulter et exporter. Aucune écriture. |
| **Production** | Scanner, créer et valider des opérations, modifier des fûts, consulter clients et lots. |
| **Administrateur** | Tout dans sa brasserie : utilisateurs, référentiels, paramètres, transitions forcées. |

Ces contrôles sont appliqués **côté serveur sur chaque route**. Un bouton
masqué n'est pas une protection ; ici, une page interdite reste interdite
même si l'on en connaît l'adresse.

## Se connecter

Adresse e-mail et mot de passe. La session dure **30 jours** sur l'appareil :
c'est voulu, taper un mot de passe fort avec des gants mouillés vingt fois
par jour est intenable.

Après 15 minutes d'inactivité, la session se **verrouille** et redemande un
code PIN à six chiffres. Le PIN ne remplace jamais le mot de passe : il
réveille une session déjà ouverte sur un appareil déjà connu. Cinq erreurs
et il faut se reconnecter complètement.

Définissez votre PIN dans **Mon compte**. C'est ce qui rend l'application
utilisable en atelier.

## Le cycle de vie d'un fût

```
(mise en service) → Reçu → Sale → Propre → Rempli → Livré ─┐
                      ▲                                     │
                      └──────────── retour brasserie ───────┘
```

| Statut | Ce qu'il veut dire |
|---|---|
| **Reçu** | de retour à la brasserie, pas encore trié. C'est aussi l'état d'un fût neuf. |
| **Sale** | vide et souillé, en attente de lavage |
| **Propre** | lavé, disponible pour un remplissage |
| **Rempli** | contient une bière, un lot, une cuve |
| **Livré** | chez un client |
| **Hors service** | sorti du parc, réparable |
| **Perdu** | disparu, sans effacer son histoire |

Les transitions incohérentes sont refusées avec un message clair : *« un
fût Sale ne peut pas partir en livraison »*. Certaines exigent des
informations : passer à **Rempli** demande une bière, un lot et une cuve ;
passer à **Livré** demande un client et une adresse ; sortir un fût du parc
demande une justification.

Un fût qui revient **plein** garde son contenu : le passage en Reçu libère
le client, pas la bière. C'est le passage en Sale qui vide tout.

### Forcer une transition

La réalité de l'atelier dépasse toujours le modèle. Un administrateur peut
donc forcer n'importe quelle transition — livrer un fût propre en échange,
par exemple. Elle exige une justification, et apparaît en rouge dans la
chronologie du fût, pour toujours. C'est le but : l'exception laisse une
trace au lieu de se fondre dans le flux normal.

## Scanner

L'écran **Scanner** accepte trois entrées, par ordre d'efficacité sur le
terrain.

1. **La douchette Bluetooth.** Une douchette 2D à une quarantaine d'euros
   se comporte exactement comme un clavier. Elle fonctionne sans rien
   configurer : le champ de saisie est toujours prêt, même après un clic
   ailleurs. C'est de loin le mode le plus rapide avec des gants.
2. **La caméra.** Elle démarre seule sur mobile, en caméra arrière. Le
   bouton *Torche* apparaît si l'appareil le permet.
3. **La saisie manuelle**, pour une étiquette abîmée.

Chaque lecture produit trois retours simultanés — couleur plein écran, son
et vibration — parce que l'opérateur ne regarde pas l'écran en continu.

- **Vert** : fût trouvé, sa fiche s'ouvre.
- **Rouge** : code inconnu. Étiquette non enregistrée, fût d'une autre
  brasserie, ou code abîmé. Le scanner reprend tout seul.

Scanner ne crée jamais un fût : **ça le retrouve**. Le parc est
pré-enregistré au moment de l'étiquetage.

## Opération par lot

C'est le cœur du gain de temps : appliquer la même chose à des dizaines de
fûts scannés à la chaîne.

**Étape 1 — Configuration.** Choisissez le type d'opération, puis ce que
vous voulez appliquer : statut, bière, lot, cuve, emplacement, client et
adresse. Les champs laissés vides ne sont pas modifiés. **Saisissez le
commentaire maintenant** : vous n'aurez plus rien à taper ensuite.

**Étape 2 — Capture.** Scannez en rafale, sans aucune validation entre deux
fûts. Le compteur est visible en permanence.

- Un fût déjà scanné déclenche un avertissement, jamais un doublon.
- Un code inconnu s'affiche en rouge, n'est pas ajouté, et le scan continue.
- **Un fût qui ne peut pas subir l'opération demandée est refusé
  immédiatement**, pendant que vous l'avez en main — pas dix minutes plus
  tard devant l'écran de validation, quand il est trop tard pour aller le
  chercher.

Chaque scan est enregistré **côté serveur**. Rafraîchissement, veille du
téléphone, batterie à plat, changement d'appareil : revenez sur la page,
votre capture est là.

**Étape 3 — Validation.** Un récapitulatif annonce « X fûts vont passer à
Y ». La validation est **transactionnelle** : soit tout passe, soit rien
n'est écrit. Si un fût a changé d'état entre-temps, l'opération entière est
refusée avec la raison, et rien n'est modifié.

Pour une livraison, le **bon de livraison imprimable** se génère aussitôt,
avec deux emplacements de signature.

## Le parc

Recherche plein texte (code QR, lot, client), filtres combinables, tri par
colonne, pagination. L'**export CSV ou Excel reflète exactement les filtres
actifs** — pas la page affichée, la vue entière.

## Fiche d'un fût

Son état courant, sa **chronologie complète** depuis la mise en service, les
autres fûts détenus par le même client, et les actions possibles depuis son
statut actuel. Rien d'autre n'est proposé : les boutons affichés sont les
transitions réellement autorisées.

## Clients

Un client se crée avec sa première adresse dans le même formulaire. Quand
un client n'a qu'une seule adresse, elle est présélectionnée partout.

**Un client qui détient des fûts ne peut pas être archivé** : récupérez-les
d'abord. Et un client ne se supprime jamais, il s'archive — une suppression
laisserait des fûts orphelins et casserait la traçabilité.

La fiche client affiche les fûts qu'il détient avec l'ancienneté de chacun.
Au-delà de 30 jours (seuil configurable), l'ancienneté passe en rouge.

## Lots et rappel produit

Un lot regroupe les fûts d'un même brassin : numéro, date de brassage,
bière, cuve, DLUO.

L'écran **Rappel produit** répond en un clic à la question qui compte en
cas d'incident : *qui a reçu le lot 2026-017 ?* Il liste tous les clients
concernés, leurs coordonnées, le nombre de fûts reçus, ceux qu'ils
détiennent encore, les dates de livraison et les codes QR exacts. Le tout
exportable et imprimable.

Les lots dont la DLUO approche remontent automatiquement sur le tableau de
bord.

## Tableau de bord

Il n'est pas le même selon qui regarde.

- **Bureau** (Administrateur, Lecture) : compteurs par statut cliquables,
  fûts non revenus au-delà du seuil, alertes DLUO, activité récente,
  répartition du parc, durée moyenne chez un client.
- **Atelier** (Production) : deux grands boutons — Scanner, Opération par
  lot — et les deux chiffres qui comptent. Tout le reste disparaît.

## Administration

- **Référentiels** — types de bière, emplacements, cuves. Ces trois listes
  alimentent toutes les listes déroulantes de l'application ; aucun de ces
  champs ne se saisit librement, ce qui garantit des statistiques justes.
- **Utilisateurs** — création avec mot de passe temporaire généré,
  changement de rôle, activation. Un administrateur ne peut ni se
  rétrograder ni se désactiver, et une brasserie conserve toujours au moins
  un administrateur actif.
- **Étiquettes QR** — générer une plage, créer les fûts correspondants,
  imprimer une planche calée sur un format 3 × 8 étiquettes de 70 × 37 mm.
- **Import de reprise** — reprendre un parc existant depuis un tableur.
  Colonnes : `code_qr`, `contenance`, et facultativement `statut`, `client`,
  `emplacement`. Le fichier est **entièrement validé avant la moindre
  écriture** : une seule erreur et rien n'est écrit, avec le détail ligne
  par ligne. L'historique passé n'existe pas : il démarre à la reprise.
- **Paramètres** — préfixe QR, emplacement par défaut, seuils d'alerte,
  contenances autorisées, fuseau horaire.
- **Journal d'audit** — qui a fait quoi, quand.

## Conseil matériel

Une étiquette QR papier ne survit pas à un lavage à la soude à 80 °C.
Prévoyez des **plaques gravées** ou des **étiquettes polyester
industrielles**. C'est le principal point de friction en exploitation
réelle, et il se règle à l'achat, pas dans le logiciel.
