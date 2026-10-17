# Guide d'utilisation

## Les trois rôles

| Rôle | Ce qu'il peut faire |
|---|---|
| **Lecture** | Consulter et exporter. Aucune écriture. |
| **Production** | Scanner, créer et valider des opérations, modifier des fûts, créer clients et lots, modifier un lot. |
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
utilisable en atelier. Le PIN se tape sur un **pavé tactile** aux grandes
touches, sans clavier système.

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
par exemple. Le forçage se fait **en deux temps** : *Forcer (admin)* ouvre
le formulaire, qui exige une justification, puis une seconde validation le
confirme. Il apparaît en rouge dans la chronologie du fût, pour toujours.
C'est le but : l'exception laisse une trace au lieu de se fondre dans le
flux normal.

## Scanner

L'écran **Scanner** accepte trois entrées, par ordre d'efficacité sur le
terrain.

1. **La douchette Bluetooth.** Une douchette 2D à une quarantaine d'euros
   se comporte exactement comme un clavier. Elle fonctionne sans rien
   configurer : le champ de saisie est toujours prêt, même après un clic
   ailleurs. C'est de loin le mode le plus rapide avec des gants.
2. **La caméra.** Elle démarre seule sur mobile, en caméra arrière. Le
   bouton *Lampe* apparaît si l'appareil le permet, *Autre caméra* si
   l'appareil en a plusieurs. Si la caméra est bloquée ou n'a pas démarré,
   *Réessayer* la relance.
3. **La saisie manuelle**, pour une étiquette abîmée : elle est **repliée**
   sous « Le code ne se lit pas ? » pour laisser la place au viseur.

Chaque lecture produit trois retours simultanés — couleur plein écran, son
et vibration — parce que l'opérateur ne regarde pas l'écran en continu.

- **Vert** : fût trouvé. Le retour est **bref** et affiche le **N°** en
  capture (en opération par lot), puis le scanner reprend tout seul.
- **Rouge ou ambre** : code inconnu, fût refusé ou déjà scanné. L'écran
  **reste affiché jusqu'au toucher** (« Touchez pour continuer ») : un
  refus ne doit jamais passer inaperçu. Avec une douchette, la lecture
  suivante le referme aussi.

Scanner ne crée jamais un fût : **ça le retrouve**. Le parc est
pré-enregistré au moment de l'étiquetage.

## Plusieurs fûts d'un coup

C'est le cœur du gain de temps : appliquer la même chose à des dizaines de
fûts scannés à la chaîne. Trois étapes, affichées en haut de l'écran :
**1. Choisir**, **2. Scanner**, **3. Confirmer**.

**1. Choisir.** Touchez un préréglage — *Lavage terminé → Propre*,
*Remplissage*, *Livraison*, *Retour client* — qui prépare le type
d'opération et le statut. Il ne reste qu'à compléter ce que le cas demande
(bière, lot et cuve pour un remplissage ; client et adresse pour une
livraison). *Autres réglages* ouvre le détail complet : statut, bière, lot,
cuve, emplacement, client, adresse. Les champs laissés vides ne sont pas
modifiés. **Saisissez le commentaire maintenant** : vous n'aurez plus rien
à taper ensuite. Depuis une fiche client ou une fiche lot, les liens
*Livrer des fûts à ce client* et *Remplir des fûts avec ce lot* arrivent
ici déjà préremplis.

**2. Scanner.** Scannez en rafale, sans aucune validation entre deux
fûts. Le compteur est visible en permanence, et *Terminer (N fûts)* passe
à la suite.

- Un fût déjà scanné déclenche un avertissement, jamais un doublon.
- Un code inconnu s'affiche en rouge, n'est pas ajouté, et le scan continue.
- **Un fût qui ne peut pas subir l'opération demandée est refusé
  immédiatement**, pendant que vous l'avez en main — pas dix minutes plus
  tard devant l'écran de validation, quand il est trop tard pour aller le
  chercher.
- *Plus d'options* propose **Tout retirer** (vider la liste) et
  **Abandonner** (jeter le brouillon). Chacun demande une confirmation en
  **deux temps** : un premier toucher arme le bouton, « Oui, … » valide.

Chaque scan est enregistré **côté serveur**. Rafraîchissement, veille du
téléphone, batterie à plat, changement d'appareil : revenez sur la page,
votre capture est là.

**3. Confirmer.** Un récapitulatif annonce « X fûts vont devenir Y ». La
validation est **transactionnelle** : soit tout passe, soit rien n'est
écrit. Si un fût a changé d'état entre-temps, l'opération entière est
refusée avec la raison, et rien n'est modifié.

Pour une livraison, le **bon de livraison imprimable** se génère aussitôt,
avec deux emplacements de signature.

## Recherche globale

Le champ de recherche du bureau (barre latérale) trouve en une saisie un
fût, un client ou un lot. Appuyez sur **/** n'importe où, hors d'un champ
de saisie, pour y placer le curseur.

## Le parc

Recherche plein texte (code QR, lot, client), filtres combinables, tri par
colonne (y compris par emplacement), pagination. Les colonnes *Dans ce
statut depuis* et *Dernier mouvement* distinguent un fût qui dort d'un fût
qui bouge. L'**export CSV ou Excel reflète exactement les filtres
actifs** — pas la page affichée, la vue entière.

## Fiche d'un fût

Elle est centrée sur **l'action suivante** : le grand bouton propose la
suite logique du cycle (un fût Sale : le laver). Les autres transitions
autorisées sont sous **Autre action…** ; rien d'autre n'est proposé, les
boutons affichés sont les transitions réellement autorisées. Suivent son
état courant, sa **chronologie complète** depuis la mise en service et les
autres fûts détenus par le même client. *Réimprimer l'étiquette* produit
une étiquette de remplacement pour ce fût.

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
bière, cuve, DLUO. La liste se filtre par **bière** et par **DLUO
proche**.

La **fiche lot** permet de le modifier (rôle Production et plus), montre
les fûts rattachés et leur répartition par statut, et propose *Remplir des
fûts avec ce lot*, qui ouvre l'opération par lot déjà préremplie.

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
  répartition du parc, durée moyenne chez un client, et des **actions
  rapides** (livrer, remplir, créer un client ou un lot) selon le rôle.
- **Atelier** (Production) : deux grands boutons — Scanner, Plusieurs
  fûts d'un coup — et les deux chiffres qui comptent. Tout le reste
  disparaît.

## Administration

Chaque sous-page propose le retour **← Administration**. Les actions
destructrices (archiver, désactiver, changer un rôle, etc.) demandent une
confirmation.

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
  `emplacement`. Un **modèle CSV** est téléchargeable depuis la page. Le fichier est **entièrement validé avant la moindre
  écriture** : une seule erreur et rien n'est écrit, avec le détail ligne
  par ligne. L'historique passé n'existe pas : il démarre à la reprise.
- **Paramètres** — préfixe QR, emplacement par défaut, seuils d'alerte,
  contenances autorisées, fuseau horaire.
- **Journal d'audit** — qui a fait quoi, quand. Filtrable par utilisateur
  et par action, paginé par 50 entrées.

## Sur téléphone

L'interface change de forme sous 720 px, elle ne se contente pas de
rétrécir :

- les listes deviennent des **cartes** — code et statut sur la première
  ligne, puis les champs renseignés, les champs vides disparaissent ;
- sur la fiche d'un fût, les **actions remontent avant la chronologie** :
  elles seraient sinon sous soixante événements ;
- les filtres secondaires du parc sont **repliés** derrière « Plus de
  filtres », pour que le premier résultat soit visible dès le premier
  écran ;
- toutes les cibles tactiles font **au moins 44 px**, et les champs de
  saisie 16 px de police — en dessous, iOS zoome tout seul à chaque clic ;
- la barre du bas dépend du rôle. **Production** : Accueil, un grand
  bouton **Scanner** central, et « Plusieurs fûts ». Administrateur :
  accueil, parc, scanner, bons, clients. Lecture : accueil, parc, bons,
  clients.

L'interface existe en thème **clair** et **sombre**, selon le réglage de
l'appareil (design « Atelier clair »).

Un outil vérifie qu'aucune page ne déborde horizontalement, sur iPhone 13,
Pixel 5 et un écran de 320 px : `outils/audit-mobile.mjs`, rejoué par
l'intégration continue.

## Conseil matériel

Une étiquette QR papier ne survit pas à un lavage à la soude à 80 °C.
Prévoyez des **plaques gravées** ou des **étiquettes polyester
industrielles**. C'est le principal point de friction en exploitation
réelle, et il se règle à l'achat, pas dans le logiciel.
