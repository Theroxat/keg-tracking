use sqlx::PgExecutor;

use crate::Resultat;

/// Alloue le numéro suivant pour une clé, par locataire, sous verrou de ligne.
/// Pas de séquence globale partagée entre brasseries : chacune a sa
/// numérotation, continue et sans trou visible.
pub async fn prochain<'e, E: PgExecutor<'e>>(ex: E, cle: &str) -> Resultat<i64> {
    Ok(sqlx::query_scalar!(
        r#"insert into compteur_reference (tenant_id, cle, valeur)
           values (locataire_courant(), $1, 1)
           on conflict (tenant_id, cle)
           do update set valeur = compteur_reference.valeur + 1
           returning valeur"#,
        cle
    )
    .fetch_one(ex)
    .await?)
}
