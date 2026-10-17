//! Accès aux données. Contient la garde qui rend l'isolation multi-locataire
//! impossible à contourner par omission.

pub mod audit;
pub mod authentification;
pub mod client;
pub mod compteur;
pub mod document;
pub mod erreur;
pub mod evenement;
pub mod fut;
pub mod garde;
pub mod locataire;
pub mod lot;
pub mod recherche;
pub mod reconstruction;
pub mod referentiel;
pub mod session;
pub mod super_admin;
pub mod utilisateur;

pub use erreur::Erreur;
pub use garde::{Depot, TransactionEditeur, TransactionLocataire};

pub type Resultat<T> = std::result::Result<T, Erreur>;

/// Migrations embarquées dans le binaire : jouées au démarrage, jamais
/// copiées à la main sur un serveur.
pub static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
