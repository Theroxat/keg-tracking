//! Cœur métier : statuts, transitions, projections.
//!
//! Ce crate ne dépend ni d'Axum, ni d'un runtime asynchrone, et n'effectue
//! aucune entrée/sortie. Il dépend de `sqlx` uniquement pour dériver la
//! correspondance entre les types fermés et les énumérations PostgreSQL.

pub mod contenance;
pub mod erreur;
pub mod evenement;
pub mod projection;
pub mod reference;
pub mod role;
pub mod statut;

pub use erreur::ErreurMetier;
pub use evenement::{EvenementApplique, Origine, TypeEvenement};
pub use projection::{appliquer, EtatFut};
pub use reference::{EtatDocument, TypeDocument};
pub use role::Role;
pub use statut::{ContexteTransition, Statut};
