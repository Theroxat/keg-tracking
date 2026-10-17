//! Sessions et authentification.
//!
//! Tout ce module travaille sur le pool `auth`, dont le rôle PostgreSQL ne
//! sait lire que `locataire`, `utilisateur` et `session`. Résoudre un cookie
//! impose de lire avant de savoir de quel locataire il s'agit ; ce rôle est
//! la réponse à ce besoin, et elle est aussi étroite que possible.

use chrono::{DateTime, Duration, Utc};
use domaine::Role;
use sqlx::PgPool;
use uuid::Uuid;

use crate::authentification::{empreinte, jeton_aleatoire};
use crate::locataire::EtatLocataire;
use crate::Resultat;

/// Session longue sur appareil de confiance : taper un mot de passe fort
/// avec des gants mouillés vingt fois par jour est intenable.
pub const DUREE_SESSION_JOURS: i64 = 30;
/// Au-delà, la session se verrouille et redemande le code PIN.
pub const INACTIVITE_VERROU_MINUTES: i64 = 15;
pub const MAX_ECHECS_CONNEXION: i32 = 5;
pub const MAX_ECHECS_PIN: i32 = 5;
pub const NOM_COOKIE: &str = "wq_session";

/// Tout ce dont une requête a besoin, en une seule lecture.
#[derive(Debug, Clone)]
pub struct ContexteSession {
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub utilisateur_id: Uuid,
    pub email: String,
    pub nom: String,
    pub prenom: String,
    pub role: Role,
    pub verrouillee: bool,
    pub a_un_pin: bool,
    pub jeton_csrf: String,
    pub derniere_activite_le: DateTime<Utc>,
    pub locataire_nom: String,
    pub locataire_etat: EtatLocataire,
    pub fuseau_horaire: String,
    /// Session ouverte par l'éditeur en assistance : bandeau permanent.
    pub usurpee_par: Option<Uuid>,
    /// Langue d'interface préférée (code ISO 639-1).
    pub langue: String,
}

impl ContexteSession {
    pub fn nom_complet(&self) -> String {
        format!("{} {}", self.prenom, self.nom).trim().to_string()
    }
    pub fn inactive_depuis_trop_longtemps(&self) -> bool {
        Utc::now() - self.derniere_activite_le > Duration::minutes(INACTIVITE_VERROU_MINUTES)
    }
}

#[derive(Debug)]
pub struct CandidatConnexion {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub mot_de_passe_hash: String,
    pub actif: bool,
    pub echecs_connexion: i32,
    pub verrouille_jusqu_a: Option<DateTime<Utc>>,
    pub locataire_etat: EtatLocataire,
}

pub async fn chercher_pour_connexion(
    pool: &PgPool,
    email: &str,
) -> Resultat<Option<CandidatConnexion>> {
    Ok(sqlx::query_as!(
        CandidatConnexion,
        r#"select u.id, u.tenant_id, u.mot_de_passe_hash, u.actif,
                  u.echecs_connexion, u.verrouille_jusqu_a,
                  l.etat as "locataire_etat: EtatLocataire"
             from utilisateur u join locataire l on l.id = u.tenant_id
            where u.email = $1::citext"#,
        email
    )
    .fetch_optional(pool)
    .await?)
}

/// Verrouillage progressif du compte : 1 min, puis 2, 4, 8… plafonné à 30.
pub async fn enregistrer_echec(pool: &PgPool, utilisateur_id: Uuid) -> Resultat<()> {
    sqlx::query!(
        r#"update utilisateur
              set echecs_connexion = echecs_connexion + 1,
                  verrouille_jusqu_a = case
                    when echecs_connexion + 1 >= $2
                    then now() + make_interval(
                           mins => least(30, power(2, echecs_connexion + 1 - $2)::int))
                    else verrouille_jusqu_a end
            where id = $1"#,
        utilisateur_id,
        MAX_ECHECS_CONNEXION
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reinitialiser_echecs(pool: &PgPool, utilisateur_id: Uuid) -> Resultat<()> {
    sqlx::query!(
        r#"update utilisateur
              set echecs_connexion = 0, verrouille_jusqu_a = null,
                  derniere_connexion_le = now()
            where id = $1"#,
        utilisateur_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Ouvre une session et renvoie le jeton en clair — la seule fois où il
/// existe hors du navigateur. La base n'en garde que l'empreinte.
pub async fn ouvrir(
    pool: &PgPool,
    tenant_id: Uuid,
    utilisateur_id: Uuid,
    appareil: Option<&str>,
    ip: Option<&str>,
) -> Resultat<String> {
    ouvrir_avec(
        pool,
        tenant_id,
        utilisateur_id,
        appareil,
        ip,
        None,
        DUREE_SESSION_JOURS,
    )
    .await
}

pub async fn ouvrir_avec(
    pool: &PgPool,
    tenant_id: Uuid,
    utilisateur_id: Uuid,
    appareil: Option<&str>,
    ip: Option<&str>,
    usurpee_par: Option<Uuid>,
    duree_jours: i64,
) -> Resultat<String> {
    let jeton = jeton_aleatoire();
    let csrf = jeton_aleatoire();
    sqlx::query!(
        r#"insert into session
             (tenant_id, utilisateur_id, jeton_hash, jeton_csrf, appareil_libelle,
              ip_creation, expire_le, usurpee_par)
           values ($1, $2, $3, $4, $5, $6::text::inet,
                   now() + make_interval(days => $7), $8)"#,
        tenant_id,
        utilisateur_id,
        empreinte(&jeton),
        csrf,
        appareil,
        ip,
        duree_jours as i32,
        usurpee_par
    )
    .execute(pool)
    .await?;
    Ok(jeton)
}

pub async fn resoudre(pool: &PgPool, jeton: &str) -> Resultat<Option<ContexteSession>> {
    Ok(sqlx::query_as!(
        ContexteSession,
        r#"select s.id as session_id, s.tenant_id, s.utilisateur_id,
                  u.email::text as "email!", u.nom, u.prenom, u.role as "role: Role",
                  s.verrouillee, (u.pin_hash is not null) as "a_un_pin!",
                  s.jeton_csrf, s.derniere_activite_le,
                  l.nom as locataire_nom, l.etat as "locataire_etat: EtatLocataire",
                  l.fuseau_horaire, s.usurpee_par, u.langue
             from session s
             join utilisateur u on u.id = s.utilisateur_id
             join locataire  l on l.id = s.tenant_id
            where s.jeton_hash = $1
              and s.revoquee_le is null
              and s.expire_le > now()
              and u.actif"#,
        empreinte(jeton)
    )
    .fetch_optional(pool)
    .await?)
}

pub async fn toucher(pool: &PgPool, session_id: Uuid) -> Resultat<()> {
    sqlx::query!(
        "update session set derniere_activite_le = now() where id = $1",
        session_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn verrouiller(pool: &PgPool, session_id: Uuid) -> Resultat<()> {
    sqlx::query!(
        "update session set verrouillee = true where id = $1",
        session_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Le PIN ne crée jamais de session : il réveille une session déjà
/// authentifiée sur un appareil déjà connu.
pub async fn hachage_pin(pool: &PgPool, utilisateur_id: Uuid) -> Resultat<Option<String>> {
    Ok(sqlx::query_scalar!(
        "select pin_hash from utilisateur where id = $1",
        utilisateur_id
    )
    .fetch_optional(pool)
    .await?
    .flatten())
}

pub async fn pin_bloque_jusqu_a(
    pool: &PgPool,
    session_id: Uuid,
) -> Resultat<Option<DateTime<Utc>>> {
    Ok(sqlx::query_scalar!(
        "select pin_bloque_jusqu_a from session where id = $1",
        session_id
    )
    .fetch_optional(pool)
    .await?
    .flatten())
}

pub async fn deverrouiller(pool: &PgPool, session_id: Uuid) -> Resultat<()> {
    sqlx::query!(
        r#"update session
              set verrouillee = false, echecs_pin = 0, pin_bloque_jusqu_a = null,
                  derniere_activite_le = now()
            where id = $1"#,
        session_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Renvoie `true` si la session a été révoquée : trop d'échecs de PIN, on
/// repasse obligatoirement par le mot de passe.
pub async fn echec_pin(pool: &PgPool, session_id: Uuid) -> Resultat<bool> {
    let echecs = sqlx::query_scalar!(
        r#"update session set echecs_pin = echecs_pin + 1,
                  pin_bloque_jusqu_a = now() + make_interval(secs => 2 * echecs_pin)
            where id = $1 returning echecs_pin"#,
        session_id
    )
    .fetch_one(pool)
    .await?;
    if echecs >= MAX_ECHECS_PIN {
        revoquer(pool, session_id).await?;
        return Ok(true);
    }
    Ok(false)
}

pub async fn revoquer(pool: &PgPool, session_id: Uuid) -> Resultat<()> {
    sqlx::query!(
        "update session set revoquee_le = now() where id = $1 and revoquee_le is null",
        session_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Utilisée quand un compte est désactivé, rétrogradé, ou que son mot de
/// passe change : toutes ses sessions tombent.
pub async fn revoquer_utilisateur(pool: &PgPool, utilisateur_id: Uuid) -> Resultat<u64> {
    Ok(sqlx::query!(
        r#"update session set revoquee_le = now()
            where utilisateur_id = $1 and revoquee_le is null"#,
        utilisateur_id
    )
    .execute(pool)
    .await?
    .rows_affected())
}

#[derive(Debug, Clone)]
pub struct SessionActive {
    pub id: Uuid,
    pub appareil_libelle: Option<String>,
    pub cree_le: DateTime<Utc>,
    pub derniere_activite_le: DateTime<Utc>,
    pub expire_le: DateTime<Utc>,
}

pub async fn lister_actives(pool: &PgPool, utilisateur_id: Uuid) -> Resultat<Vec<SessionActive>> {
    Ok(sqlx::query_as!(
        SessionActive,
        r#"select id, appareil_libelle, cree_le, derniere_activite_le, expire_le
             from session
            where utilisateur_id = $1 and revoquee_le is null and expire_le > now()
            order by derniere_activite_le desc"#,
        utilisateur_id
    )
    .fetch_all(pool)
    .await?)
}

/// Ménage périodique. Les sessions expirées depuis plus de 90 jours n'ont
/// plus d'intérêt, même pour l'audit.
pub async fn purger(pool: &PgPool) -> Resultat<u64> {
    Ok(
        sqlx::query!("delete from session where expire_le < now() - interval '90 days'")
            .execute(pool)
            .await?
            .rows_affected(),
    )
}
