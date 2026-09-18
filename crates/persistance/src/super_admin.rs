//! Back-office éditeur : hors du périmètre RLS des locataires.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::authentification::{empreinte, jeton_aleatoire};
use crate::{Erreur, Resultat};

pub const NOM_COOKIE: &str = "wq_editeur";
pub const DUREE_SESSION_HEURES: i64 = 12;

#[derive(Debug, Clone)]
pub struct SuperAdmin {
    pub id: Uuid,
    pub email: String,
    pub nom: String,
    pub actif: bool,
    pub cree_le: DateTime<Utc>,
}

pub async fn creer<'e, E: PgExecutor<'e>>(
    ex: E,
    email: &str,
    hachage: &str,
    nom: &str,
) -> Resultat<Uuid> {
    sqlx::query_scalar!(
        r#"insert into super_admin (email, mot_de_passe_hash, nom)
           values ($1::citext, $2, $3) returning id"#,
        email,
        hachage,
        nom
    )
    .fetch_one(ex)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub async fn chercher_pour_connexion(
    pool: &PgPool,
    email: &str,
) -> Resultat<Option<(Uuid, String, bool)>> {
    let r = sqlx::query!(
        r#"select id, mot_de_passe_hash, actif from super_admin where email = $1::citext"#,
        email
    )
    .fetch_optional(pool)
    .await?;
    Ok(r.map(|r| (r.id, r.mot_de_passe_hash, r.actif)))
}

pub async fn ouvrir_session(
    pool: &PgPool,
    super_admin_id: Uuid,
    ip: Option<&str>,
) -> Resultat<String> {
    let jeton = jeton_aleatoire();
    let csrf = jeton_aleatoire();
    sqlx::query!(
        r#"insert into session_super_admin
             (super_admin_id, jeton_hash, jeton_csrf, ip_creation, expire_le)
           values ($1, $2, $3, $4::text::inet, now() + make_interval(hours => $5))"#,
        super_admin_id,
        empreinte(&jeton),
        csrf,
        ip,
        DUREE_SESSION_HEURES as i32
    )
    .execute(pool)
    .await?;
    Ok(jeton)
}

#[derive(Debug, Clone)]
pub struct ContexteEditeur {
    pub session_id: Uuid,
    pub super_admin_id: Uuid,
    pub nom: String,
    pub email: String,
    pub jeton_csrf: String,
    /// Usurpation de support en cours : la session porte alors un bandeau
    /// permanent, et chaque bascule est journalisée.
    pub usurpe_locataire_id: Option<Uuid>,
}

pub async fn resoudre_session(pool: &PgPool, jeton: &str) -> Resultat<Option<ContexteEditeur>> {
    Ok(sqlx::query_as!(
        ContexteEditeur,
        r#"select s.id as session_id, s.super_admin_id, a.nom,
                  a.email::text as "email!", s.jeton_csrf, s.usurpe_locataire_id
             from session_super_admin s join super_admin a on a.id = s.super_admin_id
            where s.jeton_hash = $1 and s.revoquee_le is null
              and s.expire_le > now() and a.actif"#,
        empreinte(jeton)
    )
    .fetch_optional(pool)
    .await?)
}

pub async fn revoquer_session(pool: &PgPool, session_id: Uuid) -> Resultat<()> {
    sqlx::query!(
        "update session_super_admin set revoquee_le = now() where id = $1",
        session_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn usurper(pool: &PgPool, session_id: Uuid, locataire_id: Option<Uuid>) -> Resultat<()> {
    sqlx::query!(
        "update session_super_admin set usurpe_locataire_id = $2 where id = $1",
        session_id,
        locataire_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn tracer<'e, E: PgExecutor<'e>>(
    ex: E,
    super_admin_id: Option<Uuid>,
    action: &str,
    locataire_id: Option<Uuid>,
    details: Value,
    ip: Option<&str>,
) -> Resultat<()> {
    sqlx::query!(
        r#"insert into journal_audit_editeur
             (super_admin_id, action, locataire_id, details, ip)
           values ($1, $2, $3, $4, $5::text::inet)"#,
        super_admin_id,
        action,
        locataire_id,
        details,
        ip
    )
    .execute(ex)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct EntreeAuditEditeur {
    pub id: i64,
    pub qui: Option<String>,
    pub action: String,
    pub locataire: Option<String>,
    pub details: Value,
    pub survenu_le: DateTime<Utc>,
}

pub async fn journal<'e, E: PgExecutor<'e>>(
    ex: E,
    limite: i64,
) -> Resultat<Vec<EntreeAuditEditeur>> {
    Ok(sqlx::query_as!(
        EntreeAuditEditeur,
        r#"select j.id, a.nom as "qui?", j.action, l.nom as "locataire?",
                  j.details, j.survenu_le
             from journal_audit_editeur j
             left join super_admin a on a.id = j.super_admin_id
             left join locataire   l on l.id = j.locataire_id
            order by j.survenu_le desc limit $1"#,
        limite
    )
    .fetch_all(ex)
    .await?)
}

/// Statistiques du système, pour l'écran d'état de l'éditeur.
#[derive(Debug, Clone)]
pub struct EtatSysteme {
    pub locataires: i64,
    pub locataires_actifs: i64,
    pub utilisateurs: i64,
    pub futs: i64,
    pub evenements: i64,
}

pub async fn etat_systeme<'e, E: PgExecutor<'e>>(ex: E) -> Resultat<EtatSysteme> {
    Ok(sqlx::query_as!(
        EtatSysteme,
        r#"select
             (select count(*) from locataire)                       as "locataires!",
             (select count(*) from locataire where etat = 'actif')  as "locataires_actifs!",
             (select count(*) from utilisateur)                     as "utilisateurs!",
             (select count(*) from fut)                             as "futs!",
             (select count(*) from evenement_fut)                   as "evenements!""#
    )
    .fetch_one(ex)
    .await?)
}
