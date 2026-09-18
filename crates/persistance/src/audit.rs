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
