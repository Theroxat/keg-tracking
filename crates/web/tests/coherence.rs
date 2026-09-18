//! Cohérence client / adresse.
//!
//! Vérifiée à deux niveaux : un message applicatif clair, et une clé
//! étrangère composite qui rend l'écriture impossible même si la
//! validation était contournée.

mod commun;

use commun::{lancer, Bac, Serveur};
use domaine::{Role, Statut};
use uuid::Uuid;

struct Jeu {
    tenant: Uuid,
    admin: Uuid,
    client_a: Uuid,
    adresse_a: Uuid,
    client_b: Uuid,
    adresse_b: Uuid,
    fut: Uuid,
}

async fn preparer(bac: &Bac) -> Jeu {
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let admin = bac
        .creer_utilisateur(tenant, Role::Administrateur, "a@test.fr")
        .await;
    let mut tx = bac.depot.ouvrir(tenant, Some(admin)).await.unwrap();

    async fn faire_client(conn: &mut sqlx::PgConnection, nom: &str) -> (Uuid, Uuid) {
        let c = persistance::client::creer(
            &mut *conn,
            &persistance::client::DonneesClient {
                societe: nom,
                contact_nom: "",
                contact_prenom: "",
                telephone: "",
                email: "",
                commentaire: "",
            },
        )
        .await
        .unwrap();
        let a = persistance::client::creer_adresse(
            &mut *conn,
            c,
            &persistance::client::DonneesAdresse {
                libelle: "Principale",
                rue: "1 rue",
                code_postal: "44000",
                ville: "Nantes",
            },
        )
        .await
        .unwrap();
        (c, a)
    }
    let (client_a, adresse_a) = faire_client(tx.conn(), "Client A").await;
    let (client_b, adresse_b) = faire_client(tx.conn(), "Client B").await;

    persistance::fut::creer_en_masse(tx.conn(), &["ALPHA00001".into()], 30, None, None)
        .await
        .unwrap();
    let fut: Uuid = sqlx::query_scalar("select id from fut limit 1")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    // On l'amène jusqu'à Rempli pour qu'il puisse être livré.
    let biere = persistance::referentiel::creer_biere(tx.conn(), "Blonde", "#aabbcc", 0)
        .await
        .unwrap();
    let cuve = persistance::referentiel::creer_cuve(tx.conn(), "Cuve 1")
        .await
        .unwrap();
    let lot = persistance::lot::creer(
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
    for (statut, avec_lot) in [
        (Statut::Sale, false),
        (Statut::Propre, false),
        (Statut::Rempli, true),
    ] {
        let mut d =
            persistance::evenement::DemandeEvenement::nouvelle(fut, domaine::Origine::Systeme);
        d.statut_cible = Some(statut);
        if avec_lot {
            d.ctx.type_biere_id = Some(biere);
            d.ctx.lot_id = Some(lot);
            d.ctx.cuve_id = Some(cuve);
        }
        persistance::evenement::enregistrer(&mut tx, Role::Administrateur, &d)
            .await
            .unwrap();
    }
    tx.valider().await.unwrap();

    Jeu {
        tenant,
        admin,
        client_a,
        adresse_a,
        client_b,
        adresse_b,
        fut,
    }
}

#[tokio::test]
async fn l_adresse_d_un_autre_client_est_refusee_par_le_serveur() {
    let bac = Bac::nouveau().await;
    let j = preparer(&bac).await;
    let serveur = lancer(&bac).await;
    let (cookie, csrf) = Serveur::session(&bac, j.tenant, j.admin).await;

    // Requête forgée à la main : client A, adresse de B.
    let r = serveur
        .client
        .post(format!("{}/futs/{}/transition", serveur.base, j.fut))
        .header("Cookie", format!("wq_session={cookie}"))
        .header("Origin", "http://test.local")
        .form(&[
            ("csrf", csrf.as_str()),
            ("statut", "livre"),
            ("client_id", &j.client_a.to_string()),
            ("adresse_client_id", &j.adresse_b.to_string()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        303,
        "la réponse est une redirection avec message"
    );

    let mut tx = bac.depot.ouvrir(j.tenant, None).await.unwrap();
    let f = persistance::fut::lire(tx.conn(), j.fut).await.unwrap();
    assert_eq!(
        f.statut,
        Statut::Rempli,
        "le fût ne doit pas avoir été livré"
    );
    assert_eq!(f.client_id, None);
    tx.valider().await.unwrap();

    // Le couple cohérent, lui, passe.
    let ok = serveur
        .client
        .post(format!("{}/futs/{}/transition", serveur.base, j.fut))
        .header("Cookie", format!("wq_session={cookie}"))
        .header("Origin", "http://test.local")
        .form(&[
            ("csrf", csrf.as_str()),
            ("statut", "livre"),
            ("client_id", &j.client_a.to_string()),
            ("adresse_client_id", &j.adresse_a.to_string()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status().as_u16(), 303);
    let mut tx = bac.depot.ouvrir(j.tenant, None).await.unwrap();
    let f = persistance::fut::lire(tx.conn(), j.fut).await.unwrap();
    assert_eq!(f.statut, Statut::Livre);
    assert_eq!(f.client_id, Some(j.client_a));
    tx.valider().await.unwrap();

    let _ = j.client_b;
    bac.detruire().await;
}

#[tokio::test]
async fn la_base_refuse_le_couple_incoherent_meme_sans_validation() {
    // Défense en profondeur : si la validation applicative était
    // contournée, la clé étrangère composite refuserait quand même.
    let bac = Bac::nouveau().await;
    let j = preparer(&bac).await;
    let mut tx = bac.depot.ouvrir(j.tenant, Some(j.admin)).await.unwrap();

    let ecriture =
        sqlx::query("update fut set client_id = $1, adresse_client_id = $2 where id = $3")
            .bind(j.client_a)
            .bind(j.adresse_b)
            .bind(j.fut)
            .execute(tx.conn())
            .await;
    assert!(
        ecriture.is_err(),
        "la base doit refuser une adresse qui n'appartient pas au client"
    );
    bac.detruire().await;
}

#[tokio::test]
async fn un_client_detenteur_de_futs_ne_peut_pas_etre_archive() {
    let bac = Bac::nouveau().await;
    let j = preparer(&bac).await;
    let serveur = lancer(&bac).await;
    let (cookie, csrf) = Serveur::session(&bac, j.tenant, j.admin).await;

    // On livre le fût à A.
    {
        let mut tx = bac.depot.ouvrir(j.tenant, Some(j.admin)).await.unwrap();
        let mut d =
            persistance::evenement::DemandeEvenement::nouvelle(j.fut, domaine::Origine::Systeme);
        d.statut_cible = Some(Statut::Livre);
        d.ctx.client_id = Some(j.client_a);
        d.ctx.adresse_client_id = Some(j.adresse_a);
        persistance::evenement::enregistrer(&mut tx, Role::Administrateur, &d)
            .await
            .unwrap();
        tx.valider().await.unwrap();
    }

    let r = serveur
        .client
        .post(format!("{}/clients/{}/archiver", serveur.base, j.client_a))
        .header("Cookie", format!("wq_session={cookie}"))
        .header("Origin", "http://test.local")
        .form(&[("csrf", csrf.as_str()), ("archive", "1")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);

    let mut tx = bac.depot.ouvrir(j.tenant, None).await.unwrap();
    let cl = persistance::client::lire(tx.conn(), j.client_a)
        .await
        .unwrap();
    assert!(
        !cl.archive,
        "un client détenteur de fûts ne doit pas pouvoir être archivé"
    );
    assert_eq!(cl.futs_detenus, 1);
    tx.valider().await.unwrap();

    bac.detruire().await;
}
