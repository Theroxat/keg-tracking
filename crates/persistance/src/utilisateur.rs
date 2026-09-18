use chrono::{DateTime, Utc};
use domaine::Role;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::{Erreur, Resultat};

#[derive(Debug, Clone)]
pub struct Utilisateur {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub nom: String,
    pub prenom: String,
    pub role: Role,
    pub actif: bool,
    pub a_un_pin: bool,
    pub derniere_connexion_le: Option<DateTime<Utc>>,
    pub cree_le: DateTime<Utc>,
}

pub async fn lister<'e, E: PgExecutor<'e>>(ex: E) -> Resultat<Vec<Utilisateur>> {
    Ok(sqlx::query_as!(
        Utilisateur,
        r#"select id, tenant_id, email::text as "email!", nom, prenom,
                  role as "role: Role", actif, (pin_hash is not null) as "a_un_pin!",
                  derniere_connexion_le, cree_le
             from utilisateur order by actif desc, nom, prenom"#
    )
    .fetch_all(ex)
    .await?)
}

pub async fn lire<'e, E: PgExecutor<'e>>(ex: E, id: Uuid) -> Resultat<Utilisateur> {
    sqlx::query_as!(
        Utilisateur,
        r#"select id, tenant_id, email::text as "email!", nom, prenom,
                  role as "role: Role", actif, (pin_hash is not null) as "a_un_pin!",
                  derniere_connexion_le, cree_le
             from utilisateur where id = $1"#,
        id
    )
    .fetch_optional(ex)
    .await?
    .ok_or(Erreur::Introuvable)
}

pub struct NouvelUtilisateur<'a> {
    pub tenant_id: Uuid,
    pub email: &'a str,
    pub mot_de_passe_hash: &'a str,
    pub nom: &'a str,
    pub prenom: &'a str,
    pub role: Role,
}

pub async fn creer<'e, E: PgExecutor<'e>>(ex: E, n: &NouvelUtilisateur<'_>) -> Resultat<Uuid> {
    sqlx::query_scalar!(
        r#"insert into utilisateur (tenant_id, email, mot_de_passe_hash, nom, prenom, role)
           values ($1, $2::citext, $3, $4, $5, $6) returning id"#,
        n.tenant_id,
        n.email,
        n.mot_de_passe_hash,
        n.nom,
        n.prenom,
        n.role as Role
    )
    .fetch_one(ex)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub async fn modifier<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    nom: &str,
    prenom: &str,
    role: Role,
    actif: bool,
) -> Resultat<()> {
    sqlx::query!(
        r#"update utilisateur set nom = $2, prenom = $3, role = $4, actif = $5,
                  modifie_le = now() where id = $1"#,
        id,
        nom,
        prenom,
        role as Role,
        actif
    )
    .execute(ex)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    Ok(())
}

pub async fn changer_mot_de_passe<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    hachage: &str,
) -> Resultat<()> {
    sqlx::query!(
        r#"update utilisateur
              set mot_de_passe_hash = $2, echecs_connexion = 0,
                  verrouille_jusqu_a = null, modifie_le = now()
            where id = $1"#,
        id,
        hachage
    )
    .execute(ex)
    .await?;
    Ok(())
}

pub async fn definir_pin<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    hachage: Option<&str>,
) -> Resultat<()> {
    sqlx::query!(
        "update utilisateur set pin_hash = $2, modifie_le = now() where id = $1",
        id,
        hachage
    )
    .execute(ex)
    .await?;
    Ok(())
}

/// Nombre d'administrateurs actifs : une brasserie doit toujours en garder un.
pub async fn compter_administrateurs_actifs<'e, E: PgExecutor<'e>>(ex: E) -> Resultat<i64> {
    Ok(sqlx::query_scalar!(
        "select count(*) from utilisateur where role = 'administrateur' and actif"
    )
    .fetch_one(ex)
    .await?
    .unwrap_or(0))
}

pub async fn hachage_mot_de_passe<'e, E: PgExecutor<'e>>(ex: E, id: Uuid) -> Resultat<String> {
    sqlx::query_scalar!(
        "select mot_de_passe_hash from utilisateur where id = $1",
        id
    )
    .fetch_optional(ex)
    .await?
    .ok_or(Erreur::Introuvable)
}
