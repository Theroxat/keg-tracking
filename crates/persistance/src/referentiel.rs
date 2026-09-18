//! Types de bière, emplacements, cuves.
//!
//! Ces trois champs étaient du texte libre dans la version précédente, donc
//! pleins de fautes de frappe, donc instatistiquables. Ils sont désormais
//! administrables et utilisés partout en listes déroulantes.

use sqlx::PgExecutor;
use uuid::Uuid;

use crate::{Erreur, Resultat};

#[derive(Debug, Clone)]
pub struct TypeBiere {
    pub id: Uuid,
    pub nom: String,
    pub couleur: String,
    pub actif: bool,
    pub ordre: i32,
    pub futs: i64,
}

#[derive(Debug, Clone)]
pub struct Entree {
    pub id: Uuid,
    pub nom: String,
    pub actif: bool,
    pub futs: i64,
}

pub async fn lister_bieres<'e, E: PgExecutor<'e>>(
    ex: E,
    seulement_actifs: bool,
) -> Resultat<Vec<TypeBiere>> {
    Ok(sqlx::query_as!(
        TypeBiere,
        r#"select b.id, b.nom, b.couleur, b.actif, b.ordre,
                  (select count(*) from fut f where f.type_biere_id = b.id) as "futs!"
             from type_biere b
            where (not $1 or b.actif)
            order by b.ordre, b.nom"#,
        seulement_actifs
    )
    .fetch_all(ex)
    .await?)
}

pub async fn creer_biere<'e, E: PgExecutor<'e>>(
    ex: E,
    nom: &str,
    couleur: &str,
    ordre: i32,
) -> Resultat<Uuid> {
    sqlx::query_scalar!(
        r#"insert into type_biere (tenant_id, nom, couleur, ordre)
           values (locataire_courant(), $1, $2, $3) returning id"#,
        nom.trim(),
        couleur,
        ordre
    )
    .fetch_one(ex)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub async fn modifier_biere<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    nom: &str,
    couleur: &str,
    ordre: i32,
    actif: bool,
) -> Resultat<()> {
    sqlx::query!(
        "update type_biere set nom = $2, couleur = $3, ordre = $4, actif = $5 where id = $1",
        id,
        nom.trim(),
        couleur,
        ordre,
        actif
    )
    .execute(ex)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    Ok(())
}

pub async fn lister_emplacements<'e, E: PgExecutor<'e>>(
    ex: E,
    seulement_actifs: bool,
) -> Resultat<Vec<Entree>> {
    Ok(sqlx::query_as!(
        Entree,
        r#"select e.id, e.nom, e.actif,
                  (select count(*) from fut f where f.emplacement_id = e.id) as "futs!"
             from emplacement e where (not $1 or e.actif) order by e.nom"#,
        seulement_actifs
    )
    .fetch_all(ex)
    .await?)
}

pub async fn creer_emplacement<'e, E: PgExecutor<'e>>(ex: E, nom: &str) -> Resultat<Uuid> {
    sqlx::query_scalar!(
        "insert into emplacement (tenant_id, nom) values (locataire_courant(), $1) returning id",
        nom.trim()
    )
    .fetch_one(ex)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub async fn modifier_emplacement<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    nom: &str,
    actif: bool,
) -> Resultat<()> {
    sqlx::query!(
        "update emplacement set nom = $2, actif = $3 where id = $1",
        id,
        nom.trim(),
        actif
    )
    .execute(ex)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    Ok(())
}

pub async fn lister_cuves<'e, E: PgExecutor<'e>>(
    ex: E,
    seulement_actifs: bool,
) -> Resultat<Vec<Entree>> {
    Ok(sqlx::query_as!(
        Entree,
        r#"select c.id, c.nom, c.actif,
                  (select count(*) from fut f where f.cuve_id = c.id) as "futs!"
             from cuve c where (not $1 or c.actif) order by c.nom"#,
        seulement_actifs
    )
    .fetch_all(ex)
    .await?)
}

pub async fn creer_cuve<'e, E: PgExecutor<'e>>(ex: E, nom: &str) -> Resultat<Uuid> {
    sqlx::query_scalar!(
        "insert into cuve (tenant_id, nom) values (locataire_courant(), $1) returning id",
        nom.trim()
    )
    .fetch_one(ex)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub async fn modifier_cuve<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    nom: &str,
    actif: bool,
) -> Resultat<()> {
    sqlx::query!(
        "update cuve set nom = $2, actif = $3 where id = $1",
        id,
        nom.trim(),
        actif
    )
    .execute(ex)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    Ok(())
}
