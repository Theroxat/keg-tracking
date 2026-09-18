use domaine::ErreurMetier;

#[derive(Debug, thiserror::Error)]
pub enum Erreur {
    #[error("base de données : {0}")]
    Base(#[from] sqlx::Error),

    #[error(transparent)]
    Metier(#[from] ErreurMetier),

    #[error("introuvable")]
    Introuvable,

    #[error("conflit : {0}")]
    Conflit(String),

    #[error("hachage : {0}")]
    Hachage(String),

    #[error("configuration : {0}")]
    Configuration(String),
}

impl Erreur {
    /// Traduit une violation de contrainte PostgreSQL en conflit nommé,
    /// pour que l'interface puisse pointer le bon champ.
    pub fn depuis_contrainte(e: sqlx::Error) -> Erreur {
        if let sqlx::Error::Database(ref db) = e {
            if let Some(contrainte) = db.constraint() {
                return Erreur::Conflit(contrainte.to_string());
            }
            // Index uniques : PostgreSQL ne remonte pas toujours `constraint`.
            if db.code().as_deref() == Some("23505") {
                return Erreur::Conflit(db.message().to_string());
            }
        }
        Erreur::Base(e)
    }

    pub fn est_conflit_sur(&self, fragment: &str) -> bool {
        matches!(self, Erreur::Conflit(c) if c.contains(fragment))
    }
}
