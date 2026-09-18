//! Documents de mouvement : atomicité, dédoublonnage, refus anticipé,
//! et persistance du brouillon côté serveur.

mod commun;

use commun::Bac;
use domaine::{Role, Statut, TypeDocument};
use persistance::document::{self, Configuration, ResultatCapture};
use uuid::Uuid;

/// Prépare un locataire avec `n` fûts dans le statut demandé.
async fn parc(bac: &Bac, tenant: Uuid, n: usize, statut: Statut) -> Vec<String> {
    let codes: Vec<String> = (1..=n).map(|i| format!("ALPHA{i:05}")).collect();
    let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
    persistance::fut::creer_en_masse(tx.conn(), &codes, 30, None, None)
        .await
        .unwrap();
    if statut != Statut::Recu {
        let ids: Vec<Uuid> = sqlx::query_scalar("select id from fut order by code_qr")
            .fetch_all(tx.conn())
            .await
            .unwrap();
        for id in ids {
            let mut d =
                persistance::evenement::DemandeEvenement::nouvelle(id, domaine::Origine::Systeme);
            d.statut_cible = Some(Statut::Sale);
            persistance::evenement::enregistrer(&mut tx, Role::Administrateur, &d)
                .await
                .unwrap();
        }
    }
    tx.valider().await.unwrap();
    codes
}

#[tokio::test]
async fn la_validation_est_atomique() {
    let bac = Bac::nouveau().await;
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let admin = bac
        .creer_utilisateur(tenant, Role::Administrateur, "a@test.fr")
        .await;
    let codes = parc(&bac, tenant, 30, Statut::Sale).await;

    // Un document de lavage : Sale -> Propre, sur les 30 fûts.
    let doc_id = {
        let mut tx = bac.depot.ouvrir(tenant, Some(admin)).await.unwrap();
        let cfg = Configuration {
            statut_cible: Some(Statut::Propre),
            ..Default::default()
        };
        let id = document::creer(tx.conn(), TypeDocument::OperationInterne, &cfg, Some(admin))
            .await
            .unwrap();
        let doc = document::lire(tx.conn(), id).await.unwrap();
        for code in &codes {
            let r = document::capturer(tx.conn(), &doc, code, Role::Administrateur, Some(admin))
                .await
                .unwrap();
            assert!(
                matches!(r, ResultatCapture::Ajoute(_)),
                "{code} aurait dû être capturé"
            );
        }
        tx.valider().await.unwrap();
        id
    };

    let evenements_avant: i64 = {
        let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
        let n = sqlx::query_scalar("select count(*) from evenement_fut")
            .fetch_one(tx.conn())
            .await
            .unwrap();
        tx.valider().await.unwrap();
        n
    };

    // Entre la capture et la validation, un fût est livré par ailleurs :
    // il ne peut plus passer à Propre. C'est exactement ce qui arrive quand
    // deux opérateurs travaillent en même temps.
    {
        let mut tx = bac.depot.ouvrir(tenant, Some(admin)).await.unwrap();
        let client = persistance::client::creer(
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
        let adresse = persistance::client::creer_adresse(
            tx.conn(),
            client,
            &persistance::client::DonneesAdresse {
                libelle: "P",
                rue: "",
                code_postal: "",
                ville: "",
            },
        )
        .await
        .unwrap();
        let fut_id: Uuid = sqlx::query_scalar("select id from fut where code_qr = $1")
            .bind(&codes[16])
            .fetch_one(tx.conn())
            .await
            .unwrap();
        // Sale -> Livré n'est pas une transition normale : on la force.
        let mut d = persistance::evenement::DemandeEvenement::nouvelle(
            fut_id,
            domaine::Origine::EditionManuelle,
        );
        d.statut_cible = Some(Statut::Livre);
        d.ctx.client_id = Some(client);
        d.ctx.adresse_client_id = Some(adresse);
        d.ctx.forcee = true;
        d.ctx.justification = Some("cas de test".into());
        persistance::evenement::enregistrer(&mut tx, Role::Administrateur, &d)
            .await
            .unwrap();
        tx.valider().await.unwrap();
    }

    // La validation doit échouer, et **ne rien écrire du tout**.
    {
        let mut tx = bac.depot.ouvrir(tenant, Some(admin)).await.unwrap();
        let resultat = document::valider(&mut tx, Role::Administrateur, doc_id).await;
        assert!(resultat.is_err(), "la validation aurait dû échouer");
        // On laisse tomber la transaction, comme le fait le gestionnaire HTTP.
        drop(tx);
    }

    let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
    let apres: i64 = sqlx::query_scalar("select count(*) from evenement_fut")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    // +1 pour la livraison forcée, et rien d'autre : aucune des 30 lignes
    // n'a produit d'événement.
    assert_eq!(
        apres,
        evenements_avant + 1,
        "des événements ont été écrits malgré l'échec"
    );

    let doc = document::lire(tx.conn(), doc_id).await.unwrap();
    assert_eq!(
        doc.etat,
        domaine::EtatDocument::Brouillon,
        "le document doit rester brouillon"
    );

    let propres: i64 = sqlx::query_scalar("select count(*) from fut where statut = 'propre'")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    assert_eq!(propres, 0, "aucun fût ne doit avoir changé de statut");
    tx.valider().await.unwrap();

    bac.detruire().await;
}

#[tokio::test]
async fn la_validation_complete_ecrit_tout() {
    let bac = Bac::nouveau().await;
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let admin = bac
        .creer_utilisateur(tenant, Role::Administrateur, "a@test.fr")
        .await;
    let codes = parc(&bac, tenant, 12, Statut::Sale).await;

    let mut tx = bac.depot.ouvrir(tenant, Some(admin)).await.unwrap();
    let cfg = Configuration {
        statut_cible: Some(Statut::Propre),
        ..Default::default()
    };
    let doc_id = document::creer(tx.conn(), TypeDocument::OperationInterne, &cfg, Some(admin))
        .await
        .unwrap();
    let doc = document::lire(tx.conn(), doc_id).await.unwrap();
    for code in &codes {
        document::capturer(tx.conn(), &doc, code, Role::Administrateur, Some(admin))
            .await
            .unwrap();
    }
    let rapport = document::valider(&mut tx, Role::Administrateur, doc_id)
        .await
        .unwrap();
    assert_eq!(rapport.futs, 12);
    tx.valider().await.unwrap();

    let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
    let propres: i64 = sqlx::query_scalar("select count(*) from fut where statut = 'propre'")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    assert_eq!(propres, 12);
    // Chaque ligne pointe vers l'événement qu'elle a produit : la piste
    // d'audit relie l'opération au changement.
    let orphelines: i64 =
        sqlx::query_scalar("select count(*) from ligne_document where evenement_id is null")
            .fetch_one(tx.conn())
            .await
            .unwrap();
    assert_eq!(orphelines, 0);
    // Et le lavage est tracé sur chaque fût.
    let laves: i64 =
        sqlx::query_scalar("select count(*) from fut where dernier_lavage_le is not null")
            .fetch_one(tx.conn())
            .await
            .unwrap();
    assert_eq!(
        laves, 12,
        "le passage en Propre doit enregistrer qui a lavé et quand"
    );
    tx.valider().await.unwrap();

    bac.detruire().await;
}

#[tokio::test]
async fn dedoublonnage_et_refus_anticipe() {
    let bac = Bac::nouveau().await;
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let admin = bac
        .creer_utilisateur(tenant, Role::Administrateur, "a@test.fr")
        .await;
    let codes = parc(&bac, tenant, 4, Statut::Sale).await;

    let mut tx = bac.depot.ouvrir(tenant, Some(admin)).await.unwrap();
    // Configuration « livraison » : un fût Sale ne peut pas y aller.
    let client = persistance::client::creer(
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
    let adresse = persistance::client::creer_adresse(
        tx.conn(),
        client,
        &persistance::client::DonneesAdresse {
            libelle: "P",
            rue: "",
            code_postal: "",
            ville: "",
        },
    )
    .await
    .unwrap();
    let cfg = Configuration {
        statut_cible: Some(Statut::Livre),
        client_id: Some(client),
        adresse_client_id: Some(adresse),
        ..Default::default()
    };
    let doc_id = document::creer(tx.conn(), TypeDocument::Livraison, &cfg, Some(admin))
        .await
        .unwrap();
    let doc = document::lire(tx.conn(), doc_id).await.unwrap();

    // Refus anticipé : la transition est évaluée à la capture, pas dix
    // minutes plus tard quand le fût est reparti.
    match document::capturer(
        tx.conn(),
        &doc,
        &codes[0],
        Role::Administrateur,
        Some(admin),
    )
    .await
    .unwrap()
    {
        ResultatCapture::Refuse { raison, statut, .. } => {
            assert_eq!(statut, Statut::Sale);
            assert!(matches!(
                raison,
                domaine::ErreurMetier::TransitionInterdite { .. }
            ));
        }
        autre => panic!("le fût Sale aurait dû être refusé, obtenu {autre:?}"),
    }
    assert_eq!(
        document::lignes(tx.conn(), doc_id).await.unwrap().len(),
        0,
        "un fût refusé ne doit pas être ajouté"
    );

    // Code inconnu : erreur explicite, jamais une création.
    let inconnu = document::capturer(
        tx.conn(),
        &doc,
        "PASCHEZNOUS",
        Role::Administrateur,
        Some(admin),
    )
    .await;
    assert!(matches!(
        inconnu,
        Err(persistance::Erreur::Metier(
            domaine::ErreurMetier::CodeQrInconnu { .. }
        ))
    ));

    tx.valider().await.unwrap();

    // Dédoublonnage sur une configuration valide.
    let mut tx = bac.depot.ouvrir(tenant, Some(admin)).await.unwrap();
    let cfg = Configuration {
        statut_cible: Some(Statut::Propre),
        ..Default::default()
    };
    let doc2 = document::creer(tx.conn(), TypeDocument::OperationInterne, &cfg, Some(admin))
        .await
        .unwrap();
    let doc2 = document::lire(tx.conn(), doc2).await.unwrap();
    assert!(matches!(
        document::capturer(
            tx.conn(),
            &doc2,
            &codes[1],
            Role::Administrateur,
            Some(admin)
        )
        .await
        .unwrap(),
        ResultatCapture::Ajoute(_)
    ));
    assert!(matches!(
        document::capturer(
            tx.conn(),
            &doc2,
            &codes[1],
            Role::Administrateur,
            Some(admin)
        )
        .await
        .unwrap(),
        ResultatCapture::DejaPresent(_)
    ));
    assert_eq!(
        document::lignes(tx.conn(), doc2.id).await.unwrap().len(),
        1,
        "rescanner un fût ne doit pas créer de doublon"
    );
    tx.valider().await.unwrap();

    bac.detruire().await;
}

#[tokio::test]
async fn le_brouillon_survit_au_redemarrage() {
    // La capture est persistée côté serveur : c'est ce qui remplace une
    // couche hors-ligne. On simule la perte du processus en refermant tous
    // les pools et en repartant d'un dépôt neuf.
    let bac = Bac::nouveau().await;
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let admin = bac
        .creer_utilisateur(tenant, Role::Administrateur, "a@test.fr")
        .await;
    let codes = parc(&bac, tenant, 30, Statut::Sale).await;

    let mut tx = bac.depot.ouvrir(tenant, Some(admin)).await.unwrap();
    let cfg = Configuration {
        statut_cible: Some(Statut::Propre),
        ..Default::default()
    };
    let doc_id = document::creer(tx.conn(), TypeDocument::OperationInterne, &cfg, Some(admin))
        .await
        .unwrap();
    let doc = document::lire(tx.conn(), doc_id).await.unwrap();
    for code in &codes {
        document::capturer(tx.conn(), &doc, code, Role::Administrateur, Some(admin))
            .await
            .unwrap();
    }
    tx.valider().await.unwrap();

    // Nouveau dépôt : rien n'est conservé en mémoire.
    let depot = persistance::Depot::connecter(
        &format!(
            "postgres://webqr_app:{}@{}/{}",
            std::env::var("MDP_APP").unwrap(),
            std::env::var("SERVEUR_TEST").unwrap_or_else(|_| "localhost:5432".into()),
            bac.base
        ),
        &format!(
            "postgres://webqr_editeur:{}@{}/{}",
            std::env::var("MDP_EDITEUR").unwrap(),
            std::env::var("SERVEUR_TEST").unwrap_or_else(|_| "localhost:5432".into()),
            bac.base
        ),
        &format!(
            "postgres://webqr_auth:{}@{}/{}",
            std::env::var("MDP_AUTH").unwrap(),
            std::env::var("SERVEUR_TEST").unwrap_or_else(|_| "localhost:5432".into()),
            bac.base
        ),
        3,
    )
    .await
    .unwrap();

    let mut tx = depot.ouvrir(tenant, Some(admin)).await.unwrap();
    let repris = document::brouillon_en_cours(tx.conn(), admin)
        .await
        .unwrap()
        .expect("le brouillon doit être retrouvé");
    assert_eq!(repris.id, doc_id);
    assert_eq!(
        document::lignes(tx.conn(), doc_id).await.unwrap().len(),
        30,
        "les 30 captures doivent survivre"
    );
    tx.valider().await.unwrap();
    depot.fermer().await;

    bac.detruire().await;
}
