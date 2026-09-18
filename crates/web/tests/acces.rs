//! Contrôle d'accès par rôle, sur **chaque** route.
//!
//! Masquer un bouton n'est pas une protection : chaque route protégée est
//! appelée avec chacun des trois rôles, et doit répondre 403 quand le rôle
//! est insuffisant.

mod commun;

use commun::{lancer, Bac, Serveur};
use domaine::Role;
use uuid::Uuid;
use web::routes::INVENTAIRE;

/// Garde-fou structurel : le nombre de routes réellement enregistrées dans
/// les groupes protégés doit correspondre à l'inventaire. Ajouter une route
/// sans la déclarer fait échouer ce test.
#[test]
fn l_inventaire_couvre_toutes_les_routes_protegees() {
    const SOURCE: &str = include_str!("../src/routes/mod.rs");

    // Compte les méthodes HTTP réellement enregistrées dans les trois
    // groupes protégés. `get(a).post(b)` sur un même chemin compte pour
    // deux, ce qui est le bon grain : l'inventaire déclare un rôle par
    // couple (méthode, chemin).
    let methodes = |depuis: &str, jusqu_a: &str| -> usize {
        let debut = SOURCE
            .find(depuis)
            .unwrap_or_else(|| panic!("bloc {depuis} introuvable"));
        let reste = &SOURCE[debut..];
        let fin = reste
            .find(jusqu_a)
            .unwrap_or_else(|| panic!("fin de {depuis} introuvable"));
        let bloc = &reste[..fin];
        bloc.matches("get(").count() + bloc.matches("post(").count()
    };

    let enregistrees = methodes(
        "let lecture = Router::new()",
        "from_fn_with_state(Role::Lecture",
    ) + methodes(
        "let production = Router::new()",
        "from_fn_with_state(Role::Production",
    ) + methodes(
        "let administration = Router::new()",
        "from_fn_with_state(Role::Administrateur",
    );

    assert_eq!(
        enregistrees,
        INVENTAIRE.len(),
        "{enregistrees} méthodes enregistrées dans les groupes protégés mais {} \
         entrées dans INVENTAIRE. Toute route protégée doit y figurer avec son \
         rôle, sinon le test de contrôle d'accès ne la couvre pas.",
        INVENTAIRE.len()
    );
}

#[tokio::test]
async fn chaque_route_refuse_un_role_insuffisant() {
    let bac = Bac::nouveau().await;
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let serveur = lancer(&bac).await;

    // Un jeu de données minimal, pour que les routes paramétrées répondent
    // autre chose que 404 quand l'accès est permis.
    let (fut_id, client_id, adresse_id, lot_id, doc_id) = {
        let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
        persistance::fut::creer_en_masse(tx.conn(), &["ALPHA00001".into()], 30, None, None)
            .await
            .unwrap();
        let fut_id: Uuid = sqlx::query_scalar("select id from fut limit 1")
            .fetch_one(tx.conn())
            .await
            .unwrap();
        let client_id = persistance::client::creer(
            tx.conn(),
            &persistance::client::DonneesClient {
                societe: "Client",
                contact_nom: "",
                contact_prenom: "",
                telephone: "",
                email: "",
                commentaire: "",
            },
        )
        .await
        .unwrap();
        let adresse_id = persistance::client::creer_adresse(
            tx.conn(),
            client_id,
            &persistance::client::DonneesAdresse {
                libelle: "P",
                rue: "1 rue",
                code_postal: "44000",
                ville: "Nantes",
            },
        )
        .await
        .unwrap();
        let biere = persistance::referentiel::creer_biere(tx.conn(), "Blonde", "#aabbcc", 0)
            .await
            .unwrap();
        let cuve = persistance::referentiel::creer_cuve(tx.conn(), "Cuve 1")
            .await
            .unwrap();
        let lot_id = persistance::lot::creer(
            tx.conn(),
            &persistance::lot::DonneesLot {
                numero: "2026-001",
                date_brassage: chrono::Utc::now().date_naive(),
                type_biere_id: biere,
                cuve_id: cuve,
                dluo: None,
                commentaire: "",
            },
        )
        .await
        .unwrap();
        let doc_id = persistance::document::creer(
            tx.conn(),
            domaine::TypeDocument::OperationInterne,
            &persistance::document::Configuration::default(),
            None,
        )
        .await
        .unwrap();
        tx.valider().await.unwrap();
        (fut_id, client_id, adresse_id, lot_id, doc_id)
    };

    let mut sessions = vec![];
    for (i, role) in Role::TOUS.into_iter().enumerate() {
        let u = bac
            .creer_utilisateur(tenant, role, &format!("{}{}@test.fr", role.code(), i))
            .await;
        let (cookie, csrf) = Serveur::session(&bac, tenant, u).await;
        sessions.push((role, cookie, csrf));
    }

    let mut verifiees = 0usize;
    for (methode, chemin, role_min) in INVENTAIRE {
        let url = chemin
            .replace("{id}", &fut_id.to_string())
            .replace("{lid}", &Uuid::now_v7().to_string())
            .replace("{aid}", &adresse_id.to_string())
            .replace("{genre}", "cuve");
        // Certaines routes veulent un identifiant d'un autre type.
        let url = if chemin.starts_with("/clients/{id}") {
            chemin
                .replace("{id}", &client_id.to_string())
                .replace("{aid}", &adresse_id.to_string())
        } else if chemin.starts_with("/lots/{id}") {
            chemin.replace("{id}", &lot_id.to_string())
        } else if chemin.starts_with("/operations/{id}")
            || chemin.starts_with("/documents/{id}")
            || chemin.starts_with("/api/operations/{id}")
        {
            chemin
                .replace("{id}", &doc_id.to_string())
                .replace("{lid}", &Uuid::now_v7().to_string())
        } else {
            url
        };

        for (role, cookie, csrf) in &sessions {
            let requete = match *methode {
                "GET" => serveur.client.get(format!("{}{url}", serveur.base)),
                _ => serveur
                    .client
                    .post(format!("{}{url}", serveur.base))
                    .header("X-CSRF", csrf.as_str())
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(""),
            };
            let reponse = requete
                .header("Cookie", format!("wq_session={cookie}"))
                .header("Origin", "http://test.local")
                .send()
                .await
                .unwrap_or_else(|e| panic!("{methode} {url} : {e}"));
            let code = reponse.status().as_u16();

            if role.permet(*role_min) {
                assert_ne!(
                    code,
                    403,
                    "{methode} {url} refusé au rôle {} alors qu'il exige {}",
                    role.code(),
                    role_min.code()
                );
            } else {
                assert_eq!(
                    code,
                    403,
                    "{methode} {url} accepté pour le rôle {} alors qu'il exige {} (code {code})",
                    role.code(),
                    role_min.code()
                );
            }
            verifiees += 1;
        }
    }
    assert!(verifiees >= INVENTAIRE.len() * 3);

    bac.detruire().await;
}

#[tokio::test]
async fn sans_session_tout_est_fermé() {
    let bac = Bac::nouveau().await;
    let serveur = lancer(&bac).await;
    for (_, chemin, _) in INVENTAIRE.iter().filter(|(m, _, _)| *m == "GET") {
        let url = chemin
            .replace("{id}", &Uuid::now_v7().to_string())
            .replace("{aid}", &Uuid::now_v7().to_string())
            .replace("{genre}", "cuve");
        let r = serveur
            .client
            .get(format!("{}{url}", serveur.base))
            .send()
            .await
            .unwrap();
        assert!(
            r.status().as_u16() == 303 || r.status().as_u16() == 403,
            "{url} accessible sans session : {}",
            r.status()
        );
        if r.status().as_u16() == 303 {
            assert_eq!(
                r.headers().get("location").unwrap().to_str().unwrap(),
                "/connexion"
            );
        }
    }
    bac.detruire().await;
}

#[tokio::test]
async fn une_ecriture_sans_jeton_csrf_est_refusee() {
    let bac = Bac::nouveau().await;
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let u = bac
        .creer_utilisateur(tenant, Role::Administrateur, "a@test.fr")
        .await;
    let serveur = lancer(&bac).await;
    let (cookie, csrf) = Serveur::session(&bac, tenant, u).await;

    let sans = serveur
        .client
        .post(format!("{}/administration/referentiels/cuve", serveur.base))
        .header("Cookie", format!("wq_session={cookie}"))
        .header("Origin", "http://test.local")
        .form(&[("nom", "Cuve sans jeton")])
        .send()
        .await
        .unwrap();
    assert_eq!(
        sans.status().as_u16(),
        400,
        "une écriture sans jeton CSRF doit être refusée"
    );

    let mauvais = serveur
        .client
        .post(format!("{}/administration/referentiels/cuve", serveur.base))
        .header("Cookie", format!("wq_session={cookie}"))
        .header("Origin", "http://test.local")
        .form(&[("nom", "Cuve"), ("csrf", "jeton-invalide")])
        .send()
        .await
        .unwrap();
    assert_eq!(
        mauvais.status().as_u16(),
        400,
        "un jeton CSRF erroné doit être refusé"
    );

    // Une requête venant d'un autre site est refusée avant même le jeton.
    let etranger = serveur
        .client
        .post(format!("{}/administration/referentiels/cuve", serveur.base))
        .header("Cookie", format!("wq_session={cookie}"))
        .header("Sec-Fetch-Site", "cross-site")
        .form(&[("nom", "Cuve"), ("csrf", csrf.as_str())])
        .send()
        .await
        .unwrap();
    assert_eq!(
        etranger.status().as_u16(),
        400,
        "une requête inter-site doit être refusée"
    );

    // Et avec le bon jeton, ça passe.
    let bon = serveur
        .client
        .post(format!("{}/administration/referentiels/cuve", serveur.base))
        .header("Cookie", format!("wq_session={cookie}"))
        .header("Origin", "http://test.local")
        .form(&[("nom", "Cuve légitime"), ("csrf", csrf.as_str())])
        .send()
        .await
        .unwrap();
    assert_eq!(bon.status().as_u16(), 303);

    bac.detruire().await;
}
