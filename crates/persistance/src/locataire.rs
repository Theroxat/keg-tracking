use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::{Erreur, Resultat};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "etat_locataire", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EtatLocataire {
    Actif,
    Suspendu,
}

impl EtatLocataire {
    pub const fn cle_i18n(self) -> &'static str {
        match self {
            EtatLocataire::Actif => "locataire.actif",
            EtatLocataire::Suspendu => "locataire.suspendu",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Locataire {
    pub id: Uuid,
    pub nom: String,
    pub identifiant_court: String,
    pub prefixe_qr: String,
    pub emplacement_defaut_id: Option<Uuid>,
    pub fuseau_horaire: String,
    pub seuil_alerte_non_revenu_jours: i32,
    pub seuil_alerte_dluo_jours: i32,
    pub contenances_autorisees: Vec<i32>,
    pub etat: EtatLocataire,
    pub cree_le: DateTime<Utc>,
}

pub async fn lire<'e, E: PgExecutor<'e>>(ex: E, id: Uuid) -> Resultat<Locataire> {
    sqlx::query_as!(
        Locataire,
        r#"select id, nom, identifiant_court::text as "identifiant_court!",
                  prefixe_qr, emplacement_defaut_id, fuseau_horaire,
                  seuil_alerte_non_revenu_jours, seuil_alerte_dluo_jours,
                  contenances_autorisees, etat as "etat: EtatLocataire", cree_le
             from locataire where id = $1"#,
        id
    )
    .fetch_optional(ex)
    .await?
    .ok_or(Erreur::Introuvable)
}

pub async fn lister<'e, E: PgExecutor<'e>>(ex: E) -> Resultat<Vec<Locataire>> {
    Ok(sqlx::query_as!(
        Locataire,
        r#"select id, nom, identifiant_court::text as "identifiant_court!",
                  prefixe_qr, emplacement_defaut_id, fuseau_horaire,
                  seuil_alerte_non_revenu_jours, seuil_alerte_dluo_jours,
                  contenances_autorisees, etat as "etat: EtatLocataire", cree_le
             from locataire order by nom"#
    )
    .fetch_all(ex)
    .await?)
}

pub struct NouveauLocataire {
    pub nom: String,
    pub identifiant_court: String,
    pub prefixe_qr: String,
    pub fuseau_horaire: String,
}

pub async fn creer<'e, E: PgExecutor<'e>>(ex: E, n: &NouveauLocataire) -> Resultat<Uuid> {
    sqlx::query_scalar!(
        r#"insert into locataire (nom, identifiant_court, prefixe_qr, fuseau_horaire)
           values ($1, $2::citext, $3, $4) returning id"#,
        n.nom,
        n.identifiant_court,
        n.prefixe_qr,
        n.fuseau_horaire
    )
    .fetch_one(ex)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub struct ParametresLocataire {
    pub nom: String,
    pub prefixe_qr: String,
    pub emplacement_defaut_id: Option<Uuid>,
    pub fuseau_horaire: String,
    pub seuil_alerte_non_revenu_jours: i32,
    pub seuil_alerte_dluo_jours: i32,
    pub contenances_autorisees: Vec<i32>,
}

pub async fn enregistrer_parametres<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    p: &ParametresLocataire,
) -> Resultat<()> {
    sqlx::query!(
        r#"update locataire set nom = $2, prefixe_qr = $3, emplacement_defaut_id = $4,
                  fuseau_horaire = $5, seuil_alerte_non_revenu_jours = $6,
                  seuil_alerte_dluo_jours = $7, contenances_autorisees = $8,
                  modifie_le = now()
             where id = $1"#,
        id,
        p.nom,
        p.prefixe_qr,
        p.emplacement_defaut_id,
        p.fuseau_horaire,
        p.seuil_alerte_non_revenu_jours,
        p.seuil_alerte_dluo_jours,
        &p.contenances_autorisees
    )
    .execute(ex)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    Ok(())
}

pub async fn changer_etat<'e, E: PgExecutor<'e>>(
    ex: E,
    id: Uuid,
    etat: EtatLocataire,
) -> Resultat<()> {
    sqlx::query!(
        "update locataire set etat = $2, modifie_le = now() where id = $1",
        id,
        etat as EtatLocataire
    )
    .execute(ex)
    .await?;
    Ok(())
}
