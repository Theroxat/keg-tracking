use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::statut::Statut;

/// Nature de l'acte métier. Distinct de l'origine : un changement de statut
/// peut venir d'un scan comme d'un document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "type_evenement", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TypeEvenement {
    MiseEnService,
    Transition,
    TransitionForcee,
    Correction,
    ModificationDonnees,
    Import,
}

impl TypeEvenement {
    pub const TOUS: [TypeEvenement; 6] = [
        TypeEvenement::MiseEnService,
        TypeEvenement::Transition,
        TypeEvenement::TransitionForcee,
        TypeEvenement::Correction,
        TypeEvenement::ModificationDonnees,
        TypeEvenement::Import,
    ];

    pub const fn cle_i18n(self) -> &'static str {
        match self {
            TypeEvenement::MiseEnService => "evenement.mise_en_service",
            TypeEvenement::Transition => "evenement.transition",
            TypeEvenement::TransitionForcee => "evenement.transition_forcee",
            TypeEvenement::Correction => "evenement.correction",
            TypeEvenement::ModificationDonnees => "evenement.modification_donnees",
            TypeEvenement::Import => "evenement.import",
        }
    }
}

/// D'où vient l'écriture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "origine_evenement", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Origine {
    Scan,
    Document,
    EditionManuelle,
    Import,
    Systeme,
}

impl Origine {
    pub const fn cle_i18n(self) -> &'static str {
        match self {
            Origine::Scan => "origine.scan",
            Origine::Document => "origine.document",
            Origine::EditionManuelle => "origine.edition_manuelle",
            Origine::Import => "origine.import",
            Origine::Systeme => "origine.systeme",
        }
    }
}

/// Un événement tel qu'il est relu pour reconstruire une projection.
/// Volontairement plat : c'est la forme exacte des colonnes de
/// `evenement_fut`, et rien d'autre ne doit entrer dans le calcul.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvenementApplique {
    pub id: Uuid,
    pub sequence: i64,
    pub type_evenement: TypeEvenement,
    pub statut_apres: Option<Statut>,
    pub emplacement_id: Option<Uuid>,
    pub client_id: Option<Uuid>,
    pub adresse_client_id: Option<Uuid>,
    pub type_biere_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub cuve_id: Option<Uuid>,
    pub utilisateur_id: Option<Uuid>,
    pub survenu_le: DateTime<Utc>,
}
