use chrono::{DateTime, NaiveDate, Utc};
use domaine::{EtatFut, Origine, Statut};
use sqlx::{PgConnection, PgExecutor, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{Erreur, Resultat};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Fut {
    pub id: Uuid,
    pub code_qr: String,
    pub contenance_litres: i32,
    pub date_mise_en_service: NaiveDate,
    pub commentaire: String,
    pub statut: Statut,
    pub emplacement_id: Option<Uuid>,
    pub emplacement: Option<String>,
    pub client_id: Option<Uuid>,
    pub client: Option<String>,
    pub adresse_client_id: Option<Uuid>,
    pub type_biere_id: Option<Uuid>,
    pub biere: Option<String>,
    pub couleur_biere: Option<String>,
    pub lot_id: Option<Uuid>,
    pub lot: Option<String>,
    pub dluo: Option<NaiveDate>,
    pub cuve_id: Option<Uuid>,
    pub cuve: Option<String>,
    pub date_remplissage: Option<DateTime<Utc>>,
    pub statut_depuis_le: DateTime<Utc>,
    pub dernier_lavage_le: Option<DateTime<Utc>>,
    pub dernier_lavage_par: Option<String>,
    pub dernier_evenement_le: Option<DateTime<Utc>>,
    pub sequence_courante: i64,
    pub cree_le: DateTime<Utc>,
}

const CHAMPS_FUT: &str = r#"
  f.id, f.code_qr, f.contenance_litres, f.date_mise_en_service, f.commentaire,
  f.statut, f.emplacement_id, em.nom as emplacement,
  f.client_id, c.societe as client, f.adresse_client_id,
  f.type_biere_id, b.nom as biere, b.couleur as couleur_biere,
  f.lot_id, l.numero as lot, l.dluo,
  f.cuve_id, cv.nom as cuve,
  f.date_remplissage, f.statut_depuis_le, f.dernier_lavage_le,
  nullif(trim(coalesce(ul.prenom,'') || ' ' || coalesce(ul.nom,'')), '') as dernier_lavage_par,
  f.dernier_evenement_le, f.sequence_courante, f.cree_le
"#;

pub async fn lire<'e, E: PgExecutor<'e>>(ex: E, id: Uuid) -> Resultat<Fut> {
    sqlx::query_as!(
        Fut,
        r#"select f.id, f.code_qr, f.contenance_litres, f.date_mise_en_service,
                  f.commentaire, f.statut as "statut: Statut", f.emplacement_id,
                  em.nom as "emplacement?", f.client_id, c.societe as "client?",
                  f.adresse_client_id, f.type_biere_id, b.nom as "biere?",
                  b.couleur as "couleur_biere?", f.lot_id, l.numero as "lot?", l.dluo,
                  f.cuve_id, cv.nom as "cuve?", f.date_remplissage, f.statut_depuis_le,
                  f.dernier_lavage_le,
                  nullif(trim(coalesce(ul.prenom,'') || ' ' || coalesce(ul.nom,'')), '')
                    as "dernier_lavage_par?",
                  f.dernier_evenement_le, f.sequence_courante, f.cree_le
             from fut f
             left join emplacement em on em.id = f.emplacement_id
             left join client c       on c.id  = f.client_id
             left join type_biere b   on b.id  = f.type_biere_id
             left join lot l          on l.id  = f.lot_id
             left join cuve cv        on cv.id = f.cuve_id
             left join utilisateur ul on ul.id = f.dernier_lavage_par
            where f.id = $1"#,
        id
    )
    .fetch_optional(ex)
    .await?
    .ok_or(Erreur::Introuvable)
}

/// Scanner ne crée jamais un fût : ça le retrouve. Un code inconnu est une
/// erreur explicite, pas une création silencieuse.
pub async fn lire_par_code<'e, E: PgExecutor<'e>>(ex: E, code: &str) -> Resultat<Fut> {
    sqlx::query_as!(
        Fut,
        r#"select f.id, f.code_qr, f.contenance_litres, f.date_mise_en_service,
                  f.commentaire, f.statut as "statut: Statut", f.emplacement_id,
                  em.nom as "emplacement?", f.client_id, c.societe as "client?",
                  f.adresse_client_id, f.type_biere_id, b.nom as "biere?",
                  b.couleur as "couleur_biere?", f.lot_id, l.numero as "lot?", l.dluo,
                  f.cuve_id, cv.nom as "cuve?", f.date_remplissage, f.statut_depuis_le,
                  f.dernier_lavage_le,
                  nullif(trim(coalesce(ul.prenom,'') || ' ' || coalesce(ul.nom,'')), '')
                    as "dernier_lavage_par?",
                  f.dernier_evenement_le, f.sequence_courante, f.cree_le
             from fut f
             left join emplacement em on em.id = f.emplacement_id
             left join client c       on c.id  = f.client_id
             left join type_biere b   on b.id  = f.type_biere_id
             left join lot l          on l.id  = f.lot_id
             left join cuve cv        on cv.id = f.cuve_id
             left join utilisateur ul on ul.id = f.dernier_lavage_par
            where upper(f.code_qr) = upper($1)"#,
        code.trim()
    )
    .fetch_optional(ex)
    .await?
    .ok_or_else(|| {
        Erreur::Metier(domaine::ErreurMetier::CodeQrInconnu {
            code: code.trim().to_string(),
        })
    })
}

// ---------------------------------------------------------------- liste
#[derive(Debug, Clone, Default)]
pub struct Filtres {
    pub recherche: Option<String>,
    pub statuts: Vec<Statut>,
    pub client_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub type_biere_id: Option<Uuid>,
    pub emplacement_id: Option<Uuid>,
    pub cuve_id: Option<Uuid>,
    pub contenance: Option<i32>,
    /// Fûts chez un client depuis plus de N jours.
    pub non_revenu_jours: Option<i32>,
    /// Fûts remplis dont la DLUO tombe dans moins de N jours.
    pub dluo_dans_jours: Option<i32>,
}

impl Filtres {
    pub fn actifs(&self) -> bool {
        self.recherche.is_some()
            || !self.statuts.is_empty()
            || self.client_id.is_some()
            || self.lot_id.is_some()
            || self.type_biere_id.is_some()
            || self.emplacement_id.is_some()
            || self.cuve_id.is_some()
            || self.contenance.is_some()
            || self.non_revenu_jours.is_some()
            || self.dluo_dans_jours.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tri {
    CodeQr,
    Statut,
    Anciennete,
    DerniereActivite,
    Remplissage,
    Client,
    Contenance,
}

impl Tri {
    /// Ensemble fermé : la chaîne injectée dans le SQL ne vient jamais de
    /// l'extérieur, elle sort de ce `match`.
    const fn colonne(self) -> &'static str {
        match self {
            Tri::CodeQr => "f.code_qr",
            Tri::Statut => "f.statut",
            Tri::Anciennete => "f.statut_depuis_le",
            Tri::DerniereActivite => "f.dernier_evenement_le",
            Tri::Remplissage => "f.date_remplissage",
            Tri::Client => "c.societe",
            Tri::Contenance => "f.contenance_litres",
        }
    }
    pub const fn code(self) -> &'static str {
        match self {
            Tri::CodeQr => "code_qr",
            Tri::Statut => "statut",
            Tri::Anciennete => "anciennete",
            Tri::DerniereActivite => "activite",
            Tri::Remplissage => "remplissage",
            Tri::Client => "client",
            Tri::Contenance => "contenance",
        }
    }
    pub fn depuis_code(c: &str) -> Tri {
        match c {
            "statut" => Tri::Statut,
            "anciennete" => Tri::Anciennete,
            "activite" => Tri::DerniereActivite,
            "remplissage" => Tri::Remplissage,
            "client" => Tri::Client,
            "contenance" => Tri::Contenance,
            _ => Tri::CodeQr,
        }
    }
}

fn appliquer_filtres<'a>(qb: &mut QueryBuilder<'a, Postgres>, f: &'a Filtres) {
    qb.push(" where true");
    if let Some(r) = &f.recherche {
        qb.push(" and (f.code_qr ilike ")
            .push_bind(format!("%{r}%"))
            .push(" or c.societe ilike ")
            .push_bind(format!("%{r}%"))
            .push(" or l.numero ilike ")
            .push_bind(format!("%{r}%"))
            .push(")");
    }
    if !f.statuts.is_empty() {
        let codes: Vec<String> = f.statuts.iter().map(|s| s.code().to_string()).collect();
        qb.push(" and f.statut = any(")
            .push_bind(codes)
            .push("::text[]::statut_fut[])");
    }
    if let Some(v) = f.client_id {
        qb.push(" and f.client_id = ").push_bind(v);
    }
    if let Some(v) = f.lot_id {
        qb.push(" and f.lot_id = ").push_bind(v);
    }
    if let Some(v) = f.type_biere_id {
        qb.push(" and f.type_biere_id = ").push_bind(v);
    }
    if let Some(v) = f.emplacement_id {
        qb.push(" and f.emplacement_id = ").push_bind(v);
    }
    if let Some(v) = f.cuve_id {
        qb.push(" and f.cuve_id = ").push_bind(v);
    }
    if let Some(v) = f.contenance {
        qb.push(" and f.contenance_litres = ").push_bind(v);
    }
    if let Some(j) = f.non_revenu_jours {
        qb.push(" and f.statut = 'livre' and f.statut_depuis_le < now() - make_interval(days => ")
            .push_bind(j)
            .push(")");
    }
    if let Some(j) = f.dluo_dans_jours {
        qb.push(" and f.statut in ('rempli','livre') and l.dluo is not null and l.dluo <= current_date + make_interval(days => ")
            .push_bind(j)
            .push(")");
    }
}

const JOINTURES: &str = r#"
  from fut f
  left join emplacement em on em.id = f.emplacement_id
  left join client c       on c.id  = f.client_id
  left join type_biere b   on b.id  = f.type_biere_id
  left join lot l          on l.id  = f.lot_id
  left join cuve cv        on cv.id = f.cuve_id
  left join utilisateur ul on ul.id = f.dernier_lavage_par
"#;

/// Liste filtrée, triée et paginée.
///
/// Requête assemblée dynamiquement : c'est le seul endroit du programme qui
/// échappe à la vérification à la compilation. Les fragments SQL variables
/// proviennent tous d'ensembles fermés (`Tri`, `Statut`), jamais d'une
/// chaîne reçue du client — celles-ci passent exclusivement par `push_bind`.
pub async fn lister(
    conn: &mut PgConnection,
    f: &Filtres,
    tri: Tri,
    descendant: bool,
    page: i64,
    par_page: i64,
) -> Resultat<Vec<Fut>> {
    let mut qb = QueryBuilder::new("select ");
    qb.push(CHAMPS_FUT).push(JOINTURES);
    appliquer_filtres(&mut qb, f);
    qb.push(" order by ")
        .push(tri.colonne())
        .push(if descendant {
            " desc nulls last"
        } else {
            " asc nulls last"
        })
        .push(", f.code_qr asc limit ")
        .push_bind(par_page)
        .push(" offset ")
        .push_bind((page.max(1) - 1) * par_page);
    Ok(qb.build_query_as::<Fut>().fetch_all(&mut *conn).await?)
}

/// Plafond de l'affichage : une page d'interface ne dépasse jamais cette
/// taille, quelle que soit la valeur reçue du client.
pub const PAGE_MAX: i64 = 500;

pub async fn compter(conn: &mut PgConnection, f: &Filtres) -> Resultat<i64> {
    let mut qb = QueryBuilder::new("select count(*) ");
    qb.push(JOINTURES);
    appliquer_filtres(&mut qb, f);
    Ok(qb.build_query_scalar::<i64>().fetch_one(&mut *conn).await?)
}

/// Export : même filtre, même tri, **toutes** les lignes.
///
/// L'export doit refléter exactement la vue filtrée — c'est un besoin
/// quotidien du bureau, et un export tronqué au nombre de lignes d'un écran
/// serait un piège silencieux.
pub async fn lister_tout(
    conn: &mut PgConnection,
    f: &Filtres,
    tri: Tri,
    descendant: bool,
) -> Resultat<Vec<Fut>> {
    lister(conn, f, tri, descendant, 1, i64::MAX).await
}

// ---------------------------------------------------------------- création
pub struct NouveauFut<'a> {
    pub code_qr: &'a str,
    pub contenance_litres: i32,
    pub date_mise_en_service: NaiveDate,
    pub commentaire: &'a str,
    pub statut: Statut,
    pub emplacement_id: Option<Uuid>,
}

/// Crée un fût **et** son événement de mise en service, dans la même
/// transaction. Un fût sans histoire n'existe pas.
pub async fn creer(
    conn: &mut PgConnection,
    n: &NouveauFut<'_>,
    utilisateur_id: Option<Uuid>,
) -> Resultat<Uuid> {
    let id = sqlx::query_scalar!(
        r#"insert into fut (tenant_id, code_qr, contenance_litres, date_mise_en_service,
                            commentaire, statut, emplacement_id, cree_par, modifie_par)
           values (locataire_courant(), $1, $2, $3, $4, $5, $6, $7, $7) returning id"#,
        n.code_qr.trim(),
        n.contenance_litres,
        n.date_mise_en_service,
        n.commentaire.trim(),
        n.statut as Statut,
        n.emplacement_id,
        utilisateur_id
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(Erreur::depuis_contrainte)?;

    let mut d = crate::evenement::DemandeEvenement::nouvelle(id, Origine::Systeme);
    d.statut_cible = Some(n.statut);
    d.ctx.emplacement_id = n.emplacement_id;
    d.type_impose = Some(domaine::TypeEvenement::MiseEnService);
    d.commentaire = "Mise en service".into();
    crate::evenement::ecrire(&mut *conn, utilisateur_id, None, EtatFut::default(), &d).await?;
    Ok(id)
}

/// Mise en service en masse : N fûts numérotés séquentiellement, avec leur
/// événement, en une seule requête.
pub async fn creer_en_masse(
    conn: &mut PgConnection,
    codes: &[String],
    contenance_litres: i32,
    emplacement_id: Option<Uuid>,
    utilisateur_id: Option<Uuid>,
) -> Resultat<i64> {
    if codes.is_empty() {
        return Ok(0);
    }
    // Deux instructions, pas une seule avec des CTE : dans PostgreSQL, les
    // CTE modifiantes ne voient pas les lignes que les autres viennent
    // d'écrire. Un `update … from` placé dans le même ordre ne trouverait
    // donc aucune ligne, et la projection resterait à zéro.
    let n = sqlx::query_scalar!(
        r#"with nouveaux as (
             insert into fut (tenant_id, code_qr, contenance_litres, statut,
                              emplacement_id, cree_par, modifie_par)
             select locataire_courant(), code, $2, 'recu', $3, $4, $4
               from unnest($1::text[]) as code
             returning id, statut
           )
           insert into evenement_fut
             (id, tenant_id, fut_id, sequence, type_evenement, origine,
              statut_avant, statut_apres, emplacement_id, utilisateur_id, commentaire)
           select gen_random_uuid(), locataire_courant(), n.id, 1,
                  'mise_en_service', 'systeme', null, n.statut, $3, $4,
                  'Mise en service en masse'
             from nouveaux n
           returning 1"#,
        codes,
        contenance_litres,
        emplacement_id,
        utilisateur_id
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(Erreur::depuis_contrainte)?
    .len() as i64;

    sqlx::query!(
        r#"update fut f
              set sequence_courante = e.sequence,
                  dernier_evenement_id = e.id,
                  dernier_evenement_le = e.survenu_le,
                  statut_depuis_le = e.survenu_le
             from evenement_fut e
            where e.fut_id = f.id and e.sequence = 1
              and f.sequence_courante = 0 and f.code_qr = any($1::text[])"#,
        codes
    )
    .execute(&mut *conn)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    Ok(n)
}

pub async fn codes_deja_pris<'e, E: PgExecutor<'e>>(
    ex: E,
    codes: &[String],
) -> Resultat<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "select code_qr from fut where code_qr = any($1::text[]) order by code_qr",
        codes
    )
    .fetch_all(ex)
    .await?)
}

pub async fn modifier_identite<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    contenance_litres: i32,
    date_mise_en_service: NaiveDate,
    commentaire: &str,
    utilisateur_id: Option<Uuid>,
) -> Resultat<()> {
    sqlx::query!(
        r#"update fut set contenance_litres = $2, date_mise_en_service = $3,
                  commentaire = $4, modifie_le = now(), modifie_par = $5
            where id = $1"#,
        id,
        contenance_litres,
        date_mise_en_service,
        commentaire.trim(),
        utilisateur_id
    )
    .execute(ex)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------- pilotage
#[derive(Debug, Clone)]
pub struct CompteurStatut {
    pub statut: Statut,
    pub nombre: i64,
}

pub async fn compter_par_statut<'e, E: PgExecutor<'e>>(ex: E) -> Resultat<Vec<CompteurStatut>> {
    Ok(sqlx::query_as!(
        CompteurStatut,
        r#"select statut as "statut: Statut", count(*) as "nombre!"
             from fut group by statut"#
    )
    .fetch_all(ex)
    .await?)
}

#[derive(Debug, Clone)]
pub struct FutImmobilise {
    pub id: Uuid,
    pub code_qr: String,
    pub client_id: Option<Uuid>,
    pub client: Option<String>,
    pub depuis_le: DateTime<Utc>,
    pub jours: i32,
    pub lot: Option<String>,
}

/// Fûts chez un client depuis plus que le seuil configuré. C'est la réponse
/// à « quels fûts ne sont pas revenus ? », la question qui coûte le plus cher.
pub async fn non_revenus<'e, E: PgExecutor<'e>>(
    ex: E,
    seuil_jours: i32,
    limite: i64,
) -> Resultat<Vec<FutImmobilise>> {
    Ok(sqlx::query_as!(
        FutImmobilise,
        r#"select f.id, f.code_qr, f.client_id, c.societe as "client?",
                  f.statut_depuis_le as depuis_le,
                  extract(day from now() - f.statut_depuis_le)::int as "jours!",
                  l.numero as "lot?"
             from fut f
             left join client c on c.id = f.client_id
             left join lot l    on l.id = f.lot_id
            where f.statut = 'livre'
              and f.statut_depuis_le < now() - make_interval(days => $1)
            order by f.statut_depuis_le limit $2"#,
        seuil_jours,
        limite
    )
    .fetch_all(ex)
    .await?)
}

#[derive(Debug, Clone)]
pub struct StatsRotation {
    pub rotations: i64,
    pub duree_moyenne_chez_client_jours: Option<f64>,
    pub duree_moyenne_cycle_jours: Option<f64>,
}

/// Durée moyenne passée chez un client, calculée directement sur le flux
/// d'événements : aucune table ni aucun calcul batch n'est nécessaire.
pub async fn statistiques_rotation<'e, E: PgExecutor<'e>>(
    ex: E,
    depuis_jours: i32,
) -> Resultat<StatsRotation> {
    Ok(sqlx::query_as!(
        StatsRotation,
        r#"with sejours as (
             select e.fut_id, e.survenu_le as depart,
                    lead(e.survenu_le) over (partition by e.fut_id order by e.sequence) as retour,
                    e.statut_apres
               from evenement_fut e
              where e.statut_apres is not null
                and e.survenu_le > now() - make_interval(days => $1)
           )
           select count(*) filter (where statut_apres = 'livre' and retour is not null)
                    as "rotations!",
                  avg(extract(epoch from (retour - depart)) / 86400.0)
                    filter (where statut_apres = 'livre' and retour is not null)
                    ::float8 as "duree_moyenne_chez_client_jours",
                  avg(extract(epoch from (retour - depart)) / 86400.0)
                    filter (where statut_apres = 'propre' and retour is not null)
                    ::float8 as "duree_moyenne_cycle_jours"
             from sejours"#,
        depuis_jours
    )
    .fetch_one(ex)
    .await?)
}

/// Fûts détenus par un client, avec l'ancienneté de chacun.
pub async fn detenus_par_client<'e, E: PgExecutor<'e>>(
    ex: E,
    client_id: Uuid,
) -> Resultat<Vec<FutImmobilise>> {
    Ok(sqlx::query_as!(
        FutImmobilise,
        r#"select f.id, f.code_qr, f.client_id, c.societe as "client?",
                  f.statut_depuis_le as depuis_le,
                  extract(day from now() - f.statut_depuis_le)::int as "jours!",
                  l.numero as "lot?"
             from fut f
             left join client c on c.id = f.client_id
             left join lot l    on l.id = f.lot_id
            where f.client_id = $1 and f.statut = 'livre'
            order by f.statut_depuis_le"#,
        client_id
    )
    .fetch_all(ex)
    .await?)
}
