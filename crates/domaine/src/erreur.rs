use crate::statut::Statut;

/// Erreurs métier. Elles sont structurées, pas textuelles : le rendu en
/// français vit dans la couche de traduction (`web::i18n`), pour qu'aucune
/// chaîne visible par l'utilisateur ne soit écrite en dur ici.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErreurMetier {
    #[error("transition interdite {avant:?} -> {apres:?}")]
    TransitionInterdite { avant: Statut, apres: Statut },

    #[error("le fût est déjà dans le statut {statut:?}")]
    DejaDansCeStatut { statut: Statut },

    #[error("précondition manquante pour {statut:?} : {champ}")]
    PreconditionManquante { statut: Statut, champ: &'static str },

    #[error("justification requise")]
    JustificationRequise,

    #[error("le forçage est réservé aux administrateurs")]
    ForcageReserveAdministrateur,

    #[error("contenance non autorisée : {litres} L")]
    ContenanceNonAutorisee { litres: i32 },

    #[error("l'adresse n'appartient pas au client choisi")]
    AdresseEtrangereAuClient,

    #[error("le client est archivé")]
    ClientArchive,

    #[error("le lot ne correspond pas au type de bière choisi")]
    LotIncoherentAvecBiere,

    #[error("code QR inconnu : {code}")]
    CodeQrInconnu { code: String },

    #[error("ce fût est déjà présent dans le document")]
    FutDejaCapture,

    #[error("le document n'est plus un brouillon")]
    DocumentNonModifiable,
}

impl ErreurMetier {
    /// Clé de traduction du message affiché à l'opérateur.
    pub const fn cle_i18n(&self) -> &'static str {
        match self {
            ErreurMetier::TransitionInterdite { .. } => "erreur.transition_interdite",
            ErreurMetier::DejaDansCeStatut { .. } => "erreur.deja_dans_ce_statut",
            ErreurMetier::PreconditionManquante { .. } => "erreur.precondition_manquante",
            ErreurMetier::JustificationRequise => "erreur.justification_requise",
            ErreurMetier::ForcageReserveAdministrateur => "erreur.forcage_reserve",
            ErreurMetier::ContenanceNonAutorisee { .. } => "erreur.contenance_non_autorisee",
            ErreurMetier::AdresseEtrangereAuClient => "erreur.adresse_etrangere",
            ErreurMetier::ClientArchive => "erreur.client_archive",
            ErreurMetier::LotIncoherentAvecBiere => "erreur.lot_incoherent",
            ErreurMetier::CodeQrInconnu { .. } => "erreur.qr_inconnu",
            ErreurMetier::FutDejaCapture => "erreur.fut_deja_capture",
            ErreurMetier::DocumentNonModifiable => "erreur.document_non_modifiable",
        }
    }
}
