use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::{Erreur, Resultat};

#[derive(Debug, Clone)]
pub struct Lot {
    pub id: Uuid,
    pub numero: String,
    pub date_brassage: NaiveDate,
    pub type_biere_id: Uuid,
    pub biere: String,
    pub couleur_biere: String,
    pub cuve_id: Uuid,
    pub cuve: String,
    pub dluo: Option<NaiveDate>,
    pub commentaire: String,
    pub futs: i64,
    pub cree_le: DateTime<Utc>,
}

const SELECT: &str = "";

pub async fn lister<'e, E: PgExecutor<'e>>(ex: E, recherche: Option<&str>) -> Resultat<Vec<Lot>> {
    let _ = SELECT;
    Ok(sqlx::query_as!(
        Lot,
        r#"select l.id, l.numero, l.date_brassage, l.type_biere_id,
                  b.nom as biere, b.couleur as couleur_biere,
                  l.cuve_id, c.nom as cuve, l.dluo, l.commentaire, l.cree_le,
                  (select count(*) from fut f where f.lot_id = l.id) as "futs!"
             from lot l
             join type_biere b on b.id = l.type_biere_id
             join cuve c on c.id = l.cuve_id
            where ($1::text is null or l.numero ilike '%' || $1 || '%')
            order by l.date_brassage desc, l.numero desc"#,
        recherche
    )
    .fetch_all(ex)
    .await?)
}

pub async fn lire<'e, E: PgExecutor<'e>>(ex: E, id: Uuid) -> Resultat<Lot> {
    sqlx::query_as!(
        Lot,
        r#"select l.id, l.numero, l.date_brassage, l.type_biere_id,
                  b.nom as biere, b.couleur as couleur_biere,
                  l.cuve_id, c.nom as cuve, l.dluo, l.commentaire, l.cree_le,
                  (select count(*) from fut f where f.lot_id = l.id) as "futs!"
             from lot l
             join type_biere b on b.id = l.type_biere_id
             join cuve c on c.id = l.cuve_id
            where l.id = $1"#,
        id
    )
    .fetch_optional(ex)
    .await?
    .ok_or(Erreur::Introuvable)
}

pub struct DonneesLot<'a> {
    pub numero: &'a str,
    pub date_brassage: NaiveDate,
    pub type_biere_id: Uuid,
    pub cuve_id: Uuid,
    pub dluo: Option<NaiveDate>,
    pub commentaire: &'a str,
}

pub async fn creer<'e, E: PgExecutor<'e>>(ex: E, d: &DonneesLot<'_>) -> Resultat<Uuid> {
    sqlx::query_scalar!(
        r#"insert into lot (tenant_id, numero, date_brassage, type_biere_id, cuve_id,
                            dluo, commentaire)
           values (locataire_courant(), $1, $2, $3, $4, $5, $6) returning id"#,
        d.numero.trim(),
        d.date_brassage,
        d.type_biere_id,
        d.cuve_id,
        d.dluo,
        d.commentaire.trim()
    )
    .fetch_one(ex)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub async fn modifier<'e, E: PgExecutor<'e>>(ex: E, id: Uuid, d: &DonneesLot<'_>) -> Resultat<()> {
    sqlx::query!(
        r#"update lot set numero = $2, date_brassage = $3, type_biere_id = $4,
                  cuve_id = $5, dluo = $6, commentaire = $7 where id = $1"#,
        id,
        d.numero.trim(),
        d.date_brassage,
        d.type_biere_id,
        d.cuve_id,
        d.dluo,
        d.commentaire.trim()
    )
    .execute(ex)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    Ok(())
}

/// Rappel produit : tous les clients ayant reçu un fût de ce lot, avec la
/// date de livraison et l'état actuel du fût. C'est la fonction à valeur
/// réglementaire directe — elle a son propre écran, pas un filtre perdu.
#[derive(Debug, Clone)]
pub struct DestinataireRappel {
    pub client_id: Uuid,
    pub societe: String,
    pub contact_nom: String,
    pub telephone: String,
    pub email: String,
    pub adresse: String,
    pub futs: i64,
    pub codes_qr: Vec<String>,
    pub premiere_livraison: DateTime<Utc>,
    pub derniere_livraison: DateTime<Utc>,
    pub futs_encore_detenus: i64,
}

pub async fn rappel_produit<'e, E: PgExecutor<'e>>(
    ex: E,
    lot_id: Uuid,
) -> Resultat<Vec<DestinataireRappel>> {
    Ok(sqlx::query_as!(
        DestinataireRappel,
        r#"with livraisons as (
             select distinct on (e.fut_id, e.client_id)
                    e.fut_id, e.client_id, e.adresse_client_id, e.survenu_le
               from evenement_fut e
              where e.lot_id = $1 and e.statut_apres = 'livre' and e.client_id is not null
              order by e.fut_id, e.client_id, e.survenu_le desc
           )
           select c.id as client_id, c.societe, c.contact_nom, c.telephone, c.email,
                  coalesce(max(a.rue || ' ' || a.code_postal || ' ' || a.ville), '') as "adresse!",
                  count(distinct l.fut_id)                as "futs!",
                  array_agg(distinct f.code_qr)           as "codes_qr!",
                  min(l.survenu_le)                       as "premiere_livraison!",
                  max(l.survenu_le)                       as "derniere_livraison!",
                  count(distinct f.id) filter
                    (where f.statut = 'livre' and f.client_id = c.id
                       and f.lot_id = $1)                 as "futs_encore_detenus!"
             from livraisons l
             join client c on c.id = l.client_id
             join fut f    on f.id = l.fut_id
             left join adresse_client a on a.id = l.adresse_client_id
            group by c.id, c.societe, c.contact_nom, c.telephone, c.email
            order by c.societe"#,
        lot_id
    )
    .fetch_all(ex)
    .await?)
}

/// Lots dont la DLUO approche ou est dépassée, et qui ont encore des fûts
/// remplis ou livrés.
#[derive(Debug, Clone)]
pub struct AlerteDluo {
    pub lot_id: Uuid,
    pub numero: String,
    pub biere: String,
    pub dluo: NaiveDate,
    pub jours_restants: i32,
    pub futs_concernes: i64,
}

pub async fn alertes_dluo<'e, E: PgExecutor<'e>>(
    ex: E,
    seuil_jours: i32,
) -> Resultat<Vec<AlerteDluo>> {
    Ok(sqlx::query_as!(
        AlerteDluo,
        r#"select l.id as lot_id, l.numero, b.nom as biere, l.dluo as "dluo!",
                  (l.dluo - current_date)::int as "jours_restants!",
                  count(f.id) as "futs_concernes!"
             from lot l
             join type_biere b on b.id = l.type_biere_id
             join fut f on f.lot_id = l.id and f.statut in ('rempli', 'livre')
            where l.dluo is not null
              and l.dluo <= current_date + make_interval(days => $1)
            group by l.id, l.numero, b.nom, l.dluo
            order by l.dluo"#,
        seuil_jours
    )
    .fetch_all(ex)
    .await?)
}
