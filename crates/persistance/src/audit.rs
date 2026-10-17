use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::Resultat;

/// Journal des actions sensibles : connexions, changements de rôle,
/// suppressions, imports, usurpation de support.
pub async fn tracer<'e, E: PgExecutor<'e>>(
    ex: E,
    utilisateur_id: Option<Uuid>,
    action: &str,
    cible_type: Option<&str>,
    cible_id: Option<&str>,
    details: Value,
    ip: Option<&str>,
) -> Resultat<()> {
    sqlx::query!(
        r#"insert into journal_audit
             (tenant_id, utilisateur_id, action, cible_type, cible_id, details, ip)
           values (locataire_courant(), $1, $2, $3, $4, $5, $6::text::inet)"#,
        utilisateur_id,
        action,
        cible_type,
        cible_id,
        details,
        ip
    )
    .execute(ex)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct EntreeAudit {
    pub id: i64,
    pub utilisateur: Option<String>,
    pub action: String,
    pub cible_type: Option<String>,
    pub cible_id: Option<String>,
    pub details: Value,
    pub survenu_le: DateTime<Utc>,
}

pub async fn lister<'e, E: PgExecutor<'e>>(ex: E, limite: i64) -> Resultat<Vec<EntreeAudit>> {
    Ok(sqlx::query_as!(
        EntreeAudit,
        r#"select j.id, (u.prenom || ' ' || u.nom) as "utilisateur?", j.action,
                  j.cible_type, j.cible_id, j.details, j.survenu_le
             from journal_audit j
             left join utilisateur u on u.id = j.utilisateur_id
            order by j.survenu_le desc limit $1"#,
        limite
    )
    .fetch_all(ex)
    .await?)
}

/// Page du journal, filtrée par utilisateur et/ou action (paramètres liés).
pub async fn lister_filtre<'e, E: PgExecutor<'e>>(
    ex: E,
    utilisateur_id: Option<Uuid>,
    action: Option<&str>,
    limite: i64,
    decalage: i64,
) -> Resultat<Vec<EntreeAudit>> {
    Ok(sqlx::query_as!(
        EntreeAudit,
        r#"select j.id, (u.prenom || ' ' || u.nom) as "utilisateur?", j.action,
                  j.cible_type, j.cible_id, j.details, j.survenu_le
             from journal_audit j
             left join utilisateur u on u.id = j.utilisateur_id
            where ($1::uuid is null or j.utilisateur_id = $1)
              and ($2::text is null or j.action = $2)
            order by j.survenu_le desc, j.id desc
            limit $3 offset $4"#,
        utilisateur_id,
        action,
        limite,
        decalage
    )
    .fetch_all(ex)
    .await?)
}

pub async fn compter_filtre<'e, E: PgExecutor<'e>>(
    ex: E,
    utilisateur_id: Option<Uuid>,
    action: Option<&str>,
) -> Resultat<i64> {
    Ok(sqlx::query_scalar!(
        r#"select count(*) as "n!" from journal_audit j
            where ($1::uuid is null or j.utilisateur_id = $1)
              and ($2::text is null or j.action = $2)"#,
        utilisateur_id,
        action
    )
    .fetch_one(ex)
    .await?)
}

pub async fn actions_distinctes<'e, E: PgExecutor<'e>>(ex: E) -> Resultat<Vec<String>> {
    Ok(
        sqlx::query_scalar!(r#"select distinct action as "action!" from journal_audit order by 1"#)
            .fetch_all(ex)
            .await?,
    )
}
