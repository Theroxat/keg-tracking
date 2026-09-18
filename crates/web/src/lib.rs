pub mod erreur;
pub mod etat;
pub mod gabarits;
pub mod i18n;
pub mod middleware;
pub mod routes;
pub mod statique;

pub use erreur::ErreurWeb;
pub use etat::{Config, Etat};
pub use routes::routeur;

pub type Reponse<T = maud::Markup> = Result<T, ErreurWeb>;
