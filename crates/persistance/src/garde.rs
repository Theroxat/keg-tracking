use sqlx::{postgres::PgPoolOptions, PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::Resultat;

/// Les trois pools de l'application, chacun sur son rôle PostgreSQL.
///
/// * `app`     — soumis à la RLS. Tout le métier passe par là.
/// * `editeur` — back-office éditeur, hors périmètre des locataires.
/// * `auth`    — résolution de session et vérification de mot de passe,
///   aux droits strictement énumérés (voir migration 0009).
///
/// Le pool du propriétaire n'existe qu'au démarrage, le temps des migrations,
/// puis il est fermé.
#[derive(Clone)]
pub struct Depot {
    app: PgPool,
    editeur: PgPool,
    auth: PgPool,
}

impl Depot {
    pub async fn connecter(
        url_app: &str,
        url_editeur: &str,
        url_auth: &str,
        connexions_max: u32,
    ) -> Resultat<Depot> {
        async fn ouvrir(url: &str, max: u32) -> Result<PgPool, sqlx::Error> {
            PgPoolOptions::new()
                .max_connections(max)
                .acquire_timeout(std::time::Duration::from_secs(10))
                .connect(url)
                .await
        }
        Ok(Depot {
            app: ouvrir(url_app, connexions_max).await?,
            editeur: ouvrir(url_editeur, 4).await?,
            auth: ouvrir(url_auth, connexions_max.min(8)).await?,
        })
    }

    /// Ouvre une transaction déjà rattachée à un locataire.
    /// **C'est le seul moyen d'obtenir une connexion applicative.**
    pub async fn ouvrir(
        &self,
        locataire_id: Uuid,
        utilisateur_id: Option<Uuid>,
    ) -> Resultat<TransactionLocataire> {
        let mut tx = self.app.begin().await?;
        sqlx::query("select set_config('app.locataire_id', $1, true)")
            .bind(locataire_id.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("select set_config('app.utilisateur_id', $1, true)")
            .bind(utilisateur_id.map(|u| u.to_string()).unwrap_or_default())
            .execute(&mut *tx)
            .await?;
        Ok(TransactionLocataire {
            tx,
            locataire_id,
            utilisateur_id,
        })
    }

    /// Transaction du back-office éditeur. Voit tous les locataires.
    pub async fn ouvrir_editeur(&self) -> Resultat<TransactionEditeur> {
        Ok(TransactionEditeur {
            tx: self.editeur.begin().await?,
        })
    }

    /// Connexion d'authentification. Réservée à `authentification` et
    /// `session` : elle ne sait lire ni le parc, ni les clients, ni les lots.
    pub fn auth(&self) -> &PgPool {
        &self.auth
    }

    /// Pool du back-office éditeur, utilisé pour résoudre sa propre session.
    /// Le rôle `webqr_auth` n'a délibérément aucun droit sur les tables de
    /// l'éditeur : c'est la contrepartie de sa capacité à lire en travers
    /// des locataires.
    pub fn auth_editeur(&self) -> &PgPool {
        &self.editeur
    }

    pub async fn fermer(&self) {
        self.app.close().await;
        self.editeur.close().await;
        self.auth.close().await;
    }

    /// Sonde de santé : une requête triviale sur chaque pool.
    pub async fn en_bonne_sante(&self) -> bool {
        sqlx::query_scalar::<_, i32>("select 1")
            .fetch_one(&self.app)
            .await
            .is_ok()
    }
}

/// Transaction dont le locataire courant est déjà positionné.
///
/// Le champ `tx` est privé et `ouvrir` est le seul constructeur : il n'existe
/// aucun chemin, dans tout le programme, permettant d'obtenir une connexion
/// applicative sans avoir déclaré de quel locataire il s'agit. Un `where`
/// oublié ne retourne alors aucune ligne, au lieu de toutes.
pub struct TransactionLocataire {
    tx: Transaction<'static, Postgres>,
    locataire_id: Uuid,
    utilisateur_id: Option<Uuid>,
}

impl TransactionLocataire {
    pub fn conn(&mut self) -> &mut PgConnection {
        &mut self.tx
    }
    pub fn locataire_id(&self) -> Uuid {
        self.locataire_id
    }
    pub fn utilisateur_id(&self) -> Option<Uuid> {
        self.utilisateur_id
    }
    pub async fn valider(self) -> Resultat<()> {
        self.tx.commit().await?;
        Ok(())
    }
    pub async fn annuler(self) -> Resultat<()> {
        self.tx.rollback().await?;
        Ok(())
    }
}

pub struct TransactionEditeur {
    tx: Transaction<'static, Postgres>,
}

impl TransactionEditeur {
    pub fn conn(&mut self) -> &mut PgConnection {
        &mut self.tx
    }
    pub async fn valider(self) -> Resultat<()> {
        self.tx.commit().await?;
        Ok(())
    }
}

/// Joue les migrations avec le rôle propriétaire, puis referme aussitôt.
/// Un verrou consultatif évite que deux instances ne les jouent ensemble.
pub async fn migrer(url_owner: &str) -> Resultat<()> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(url_owner)
        .await?;
    crate::MIGRATIONS
        .run(&pool)
        .await
        .map_err(|e| Erreur::Configuration(format!("migration : {e}")))?;
    pool.close().await;
    Ok(())
}

use crate::Erreur;
