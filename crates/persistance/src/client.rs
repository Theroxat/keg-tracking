use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::{Erreur, Resultat};

#[derive(Debug, Clone)]
pub struct Client {
    pub id: Uuid,
    pub societe: String,
    pub contact_nom: String,
    pub contact_prenom: String,
    pub telephone: String,
    pub email: String,
    pub commentaire: String,
    pub archive: bool,
    pub cree_le: DateTime<Utc>,
    pub futs_detenus: i64,
    pub adresses: i64,
}

#[derive(Debug, Clone)]
pub struct Adresse {
    pub id: Uuid,
    pub client_id: Uuid,
    pub libelle: String,
    pub rue: String,
    pub code_postal: String,
    pub ville: String,
    pub archive: bool,
}

impl Adresse {
    pub fn une_ligne(&self) -> String {
        let mut parties = vec![];
        if !self.rue.is_empty() {
            parties.push(self.rue.clone());
        }
        let cp_ville = format!("{} {}", self.code_postal, self.ville);
        if !cp_ville.trim().is_empty() {
            parties.push(cp_ville.trim().to_string());
        }
        parties.join(", ")
    }
}

pub async fn lister<'e, E: PgExecutor<'e>>(
    ex: E,
    inclure_archives: bool,
    recherche: Option<&str>,
) -> Resultat<Vec<Client>> {
    Ok(sqlx::query_as!(
        Client,
        r#"select c.id, c.societe, c.contact_nom, c.contact_prenom, c.telephone,
                  c.email, c.commentaire, c.archive, c.cree_le,
                  (select count(*) from fut f
                    where f.client_id = c.id and f.statut = 'livre')  as "futs_detenus!",
                  (select count(*) from adresse_client a
                    where a.client_id = c.id and not a.archive)       as "adresses!"
             from client c
            where ($1 or not c.archive)
              and ($2::text is null or c.societe ilike '%' || $2 || '%'
                   or c.contact_nom ilike '%' || $2 || '%')
            order by c.archive, c.societe"#,
        inclure_archives,
        recherche
    )
    .fetch_all(ex)
    .await?)
}

pub async fn lire<'e, E: PgExecutor<'e>>(ex: E, id: Uuid) -> Resultat<Client> {
    sqlx::query_as!(
        Client,
        r#"select c.id, c.societe, c.contact_nom, c.contact_prenom, c.telephone,
                  c.email, c.commentaire, c.archive, c.cree_le,
                  (select count(*) from fut f
                    where f.client_id = c.id and f.statut = 'livre') as "futs_detenus!",
                  (select count(*) from adresse_client a
                    where a.client_id = c.id and not a.archive)      as "adresses!"
             from client c where c.id = $1"#,
        id
    )
    .fetch_optional(ex)
    .await?
    .ok_or(Erreur::Introuvable)
}

pub struct DonneesClient<'a> {
    pub societe: &'a str,
    pub contact_nom: &'a str,
    pub contact_prenom: &'a str,
    pub telephone: &'a str,
    pub email: &'a str,
    pub commentaire: &'a str,
}

pub async fn creer<'e, E: PgExecutor<'e>>(ex: E, d: &DonneesClient<'_>) -> Resultat<Uuid> {
    sqlx::query_scalar!(
        r#"insert into client (tenant_id, societe, contact_nom, contact_prenom,
                               telephone, email, commentaire)
           values (locataire_courant(), $1, $2, $3, $4, $5, $6) returning id"#,
        d.societe.trim(),
        d.contact_nom.trim(),
        d.contact_prenom.trim(),
        d.telephone.trim(),
        d.email.trim(),
        d.commentaire.trim()
    )
    .fetch_one(ex)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub async fn modifier<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    d: &DonneesClient<'_>,
) -> Resultat<()> {
    sqlx::query!(
        r#"update client set societe = $2, contact_nom = $3, contact_prenom = $4,
                  telephone = $5, email = $6, commentaire = $7, modifie_le = now()
            where id = $1"#,
        id,
        d.societe.trim(),
        d.contact_nom.trim(),
        d.contact_prenom.trim(),
        d.telephone.trim(),
        d.email.trim(),
        d.commentaire.trim()
    )
    .execute(ex)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    Ok(())
}

/// Un client détenteur de fûts ne se supprime pas : on l'archive. Une
/// suppression dure laisserait des fûts orphelins et casserait la traçabilité.
pub async fn archiver<'e, E: PgExecutor<'e>>(ex: E, id: Uuid, archive: bool) -> Resultat<()> {
    sqlx::query!(
        "update client set archive = $2, modifie_le = now() where id = $1",
        id,
        archive
    )
    .execute(ex)
    .await?;
    Ok(())
}

pub async fn lister_adresses<'e, E: PgExecutor<'e>>(
    ex: E,
    client_id: Uuid,
    inclure_archives: bool,
) -> Resultat<Vec<Adresse>> {
    Ok(sqlx::query_as!(
        Adresse,
        r#"select id, client_id, libelle, rue, code_postal, ville, archive
             from adresse_client
            where client_id = $1 and ($2 or not archive)
            order by archive, libelle"#,
        client_id,
        inclure_archives
    )
    .fetch_all(ex)
    .await?)
}

pub async fn lire_adresse<'e, E: PgExecutor<'e>>(ex: E, id: Uuid) -> Resultat<Adresse> {
    sqlx::query_as!(
        Adresse,
        r#"select id, client_id, libelle, rue, code_postal, ville, archive
             from adresse_client where id = $1"#,
        id
    )
    .fetch_optional(ex)
    .await?
    .ok_or(Erreur::Introuvable)
}

pub struct DonneesAdresse<'a> {
    pub libelle: &'a str,
    pub rue: &'a str,
    pub code_postal: &'a str,
    pub ville: &'a str,
}

pub async fn creer_adresse<'e, E: PgExecutor<'e>>(
    ex: E,
    client_id: Uuid,
    d: &DonneesAdresse<'_>,
) -> Resultat<Uuid> {
    sqlx::query_scalar!(
        r#"insert into adresse_client (tenant_id, client_id, libelle, rue, code_postal, ville)
           values (locataire_courant(), $1, $2, $3, $4, $5) returning id"#,
        client_id,
        if d.libelle.trim().is_empty() {
            "Principale"
        } else {
            d.libelle.trim()
        },
        d.rue.trim(),
        d.code_postal.trim(),
        d.ville.trim()
    )
    .fetch_one(ex)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub async fn modifier_adresse<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    d: &DonneesAdresse<'_>,
    archive: bool,
) -> Resultat<()> {
    sqlx::query!(
        r#"update adresse_client set libelle = $2, rue = $3, code_postal = $4,
                  ville = $5, archive = $6 where id = $1"#,
        id,
        d.libelle.trim(),
        d.rue.trim(),
        d.code_postal.trim(),
        d.ville.trim(),
        archive
    )
    .execute(ex)
    .await?;
    Ok(())
}

/// Vérification serveur de la cohérence client / adresse.
///
/// La base la garantit déjà par clé étrangère composite ; cette fonction
/// existe pour produire un message compréhensible avant d'écrire, plutôt
/// qu'une violation de contrainte brute.
pub async fn adresse_appartient<'e, E: PgExecutor<'e>>(
    ex: E,
    client_id: Uuid,
    adresse_id: Uuid,
) -> Resultat<bool> {
    Ok(sqlx::query_scalar!(
        "select exists(select 1 from adresse_client where id = $1 and client_id = $2)",
        adresse_id,
        client_id
    )
    .fetch_one(ex)
    .await?
    .unwrap_or(false))
}

/// Quand un client n'a qu'une seule adresse active, elle est présélectionnée
/// automatiquement partout.
pub async fn adresse_unique<'e, E: PgExecutor<'e>>(
    ex: E,
    client_id: Uuid,
) -> Resultat<Option<Uuid>> {
    let mut ids = sqlx::query_scalar!(
        "select id from adresse_client where client_id = $1 and not archive limit 2",
        client_id
    )
    .fetch_all(ex)
    .await?;
    if ids.len() == 1 {
        Ok(Some(ids.remove(0)))
    } else {
        Ok(None)
    }
}
