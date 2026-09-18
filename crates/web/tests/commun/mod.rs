#![allow(dead_code)] // chaque binaire de test n'utilise qu'une partie du bac à sable

//! Bac à sable de test : une base PostgreSQL neuve par test, migrée, avec
//! les trois rôles applicatifs réels.
//!
//! Les tests attaquent donc exactement la configuration de production —
//! notamment le fait que `webqr_app` n'est pas propriétaire des tables, ce
//! qui est la condition pour que la Row Level Security s'applique.

use std::sync::atomic::{AtomicU32, Ordering};

use domaine::Role;
use persistance::authentification::Hacheur;
use persistance::Depot;
use sqlx::{Connection, Executor, PgConnection};
use uuid::Uuid;

static COMPTEUR: AtomicU32 = AtomicU32::new(0);

fn serveur() -> String {
    std::env::var("SERVEUR_TEST").unwrap_or_else(|_| "localhost:5432".into())
}
fn mdp(nom: &str) -> String {
    std::env::var(nom)
        .unwrap_or_else(|_| panic!("variable {nom} absente de l'environnement de test"))
}

pub struct Bac {
    pub base: String,
    pub depot: Depot,
    pub hacheur: Hacheur,
    pub url_owner: String,
}

impl Bac {
    pub async fn nouveau() -> Bac {
        let n = COMPTEUR.fetch_add(1, Ordering::SeqCst);
        let base = format!("webqr_test_{}_{}", std::process::id(), n);
        let racine = format!(
            "postgres://webqr_owner:{}@{}/webqr",
            mdp("MDP_OWNER"),
            serveur()
        );

        let mut admin = PgConnection::connect(&racine).await.expect("connexion");
        admin
            .execute(format!(r#"drop database if exists "{base}""#).as_str())
            .await
            .expect("drop");
        admin
            .execute(format!(r#"create database "{base}""#).as_str())
            .await
            .expect("create database");
        drop(admin);

        let url_owner = format!(
            "postgres://webqr_owner:{}@{}/{}",
            mdp("MDP_OWNER"),
            serveur(),
            base
        );
        persistance::garde::migrer(&url_owner)
            .await
            .expect("migrations");

        let depot = Depot::connecter(
            &format!(
                "postgres://webqr_app:{}@{}/{}",
                mdp("MDP_APP"),
                serveur(),
                base
            ),
            &format!(
                "postgres://webqr_editeur:{}@{}/{}",
                mdp("MDP_EDITEUR"),
                serveur(),
                base
            ),
            &format!(
                "postgres://webqr_auth:{}@{}/{}",
                mdp("MDP_AUTH"),
                serveur(),
                base
            ),
            5,
        )
        .await
        .expect("pools");

        Bac {
            base,
            depot,
            hacheur: Hacheur::nouveau(b"secret-de-test-0123456789abcdefgh").unwrap(),
            url_owner,
        }
    }

    /// Connexion directe en tant que rôle applicatif, sans passer par la
    /// garde : c'est ce que le test d'isolation doit pouvoir faire pour
    /// prouver que la protection ne dépend pas de la discipline du code.
    pub async fn connexion_app(&self) -> PgConnection {
        PgConnection::connect(&format!(
            "postgres://webqr_app:{}@{}/{}",
            mdp("MDP_APP"),
            serveur(),
            self.base
        ))
        .await
        .expect("connexion app")
    }

    pub async fn connexion_owner(&self) -> PgConnection {
        PgConnection::connect(&self.url_owner)
            .await
            .expect("connexion owner")
    }

    pub async fn creer_locataire(&self, slug: &str, prefixe: &str) -> Uuid {
        let mut tx = self.depot.ouvrir_editeur().await.unwrap();
        let id = persistance::locataire::creer(
            tx.conn(),
            &persistance::locataire::NouveauLocataire {
                nom: format!("Brasserie {slug}"),
                identifiant_court: slug.into(),
                prefixe_qr: prefixe.into(),
                fuseau_horaire: "Europe/Paris".into(),
            },
        )
        .await
        .unwrap();
        tx.valider().await.unwrap();

        let mut tx = self.depot.ouvrir(id, None).await.unwrap();
        let empl = persistance::referentiel::creer_emplacement(tx.conn(), "Brasserie")
            .await
            .unwrap();
        sqlx::query("update locataire set emplacement_defaut_id = $1 where id = $2")
            .bind(empl)
            .bind(id)
            .execute(tx.conn())
            .await
            .unwrap();
        tx.valider().await.unwrap();
        id
    }

    pub async fn creer_utilisateur(&self, tenant: Uuid, role: Role, email: &str) -> Uuid {
        let hachage = self.hacheur.hacher("Motdepasse-Test-2026!").unwrap();
        let mut tx = self.depot.ouvrir(tenant, None).await.unwrap();
        let id = persistance::utilisateur::creer(
            tx.conn(),
            &persistance::utilisateur::NouvelUtilisateur {
                tenant_id: tenant,
                email,
                mot_de_passe_hash: &hachage,
                nom: "Test",
                prenom: role.code(),
                role,
            },
        )
        .await
        .unwrap();
        tx.valider().await.unwrap();
        id
    }

    pub async fn detruire(self) {
        let base = self.base.clone();
        self.depot.fermer().await;
        let racine = format!(
            "postgres://webqr_owner:{}@{}/webqr",
            mdp("MDP_OWNER"),
            serveur()
        );
        if let Ok(mut c) = PgConnection::connect(&racine).await {
            let _ = c
                .execute(format!(r#"drop database if exists "{base}" with (force)"#).as_str())
                .await;
        }
    }
}

/// Toutes les tables métier, c'est-à-dire celles qui portent `tenant_id`.
/// La liste est **lue dans le catalogue**, jamais écrite à la main : une
/// table ajoutée plus tard sans politique RLS fait donc échouer les tests
/// sans que personne n'ait pensé à les mettre à jour.
pub async fn tables_metier(conn: &mut PgConnection) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        r#"select c.relname
             from pg_class c
             join pg_namespace n on n.oid = c.relnamespace
             join pg_attribute a on a.attrelid = c.oid
                                and a.attname = 'tenant_id'
                                and not a.attisdropped
            where n.nspname = 'public' and c.relkind = 'r'
            order by c.relname"#,
    )
    .fetch_all(conn)
    .await
    .expect("catalogue")
}

// ------------------------------------------------------------- serveur HTTP
use std::sync::Arc;

pub struct Serveur {
    pub base: String,
    pub client: reqwest::Client,
}

impl Serveur {
    /// Ouvre une session réelle pour un utilisateur et renvoie le couple
    /// (cookie, jeton CSRF) à poser sur les requêtes.
    pub async fn session(bac: &Bac, tenant: Uuid, utilisateur: Uuid) -> (String, String) {
        let jeton = persistance::session::ouvrir(bac.depot.auth(), tenant, utilisateur, None, None)
            .await
            .unwrap();
        let ctx = persistance::session::resoudre(bac.depot.auth(), &jeton)
            .await
            .unwrap()
            .unwrap();
        (jeton, ctx.jeton_csrf)
    }
}

pub async fn lancer(bac: &Bac) -> Serveur {
    let config = web::Config {
        url_owner: bac.url_owner.clone(),
        url_app: String::new(),
        url_editeur: String::new(),
        url_auth: String::new(),
        secret_app: "secret-de-test-0123456789abcdefgh".into(),
        adresse_ecoute: "127.0.0.1:0".parse().unwrap(),
        domaine_public: "test.local".into(),
        fuseau_defaut: "Europe/Paris".into(),
        derriere_proxy: false,
        // En test on parle en clair : sinon le client HTTP refuse de
        // renvoyer un cookie marqué Secure.
        cookies_securises: false,
        connexions_max: 5,
    };
    let etat = web::Etat {
        depot: bac.depot.clone(),
        hacheur: bac.hacheur.clone(),
        config: Arc::new(config),
        debit: Arc::new(web::etat::Debit::nouveau()),
    };
    let ecoute = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let adresse = ecoute.local_addr().unwrap();
    let app = web::routeur(etat);
    tokio::spawn(async move {
        axum::serve(
            ecoute,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .ok();
    });
    Serveur {
        base: format!("http://{adresse}"),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    }
}
