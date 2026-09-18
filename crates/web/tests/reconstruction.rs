//! Reconstruction des projections.
//!
//! Rejouer le flux d'événements doit redonner **exactement** l'état courant.
//! C'est à la fois un filet de sécurité en production et le meilleur test
//! de cohérence du modèle.

mod commun;

use commun::Bac;
use domaine::{Role, Statut};
use uuid::Uuid;

async fn histoire_credible(bac: &Bac, tenant: Uuid, admin: Uuid, futs: usize) {
    let codes: Vec<String> = (1..=futs).map(|i| format!("ALPHA{i:05}")).collect();
    let mut tx = bac.depot.ouvrir(tenant, Some(admin)).await.unwrap();
    persistance::fut::creer_en_masse(tx.conn(), &codes, 30, None, None)
        .await
        .unwrap();

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

    let ids: Vec<Uuid> = sqlx::query_scalar("select id from fut order by code_qr")
        .fetch_all(tx.conn())
        .await
        .unwrap();
    for (i, id) in ids.iter().enumerate() {
        // Chaque fût s'arrête à une étape différente du cycle : toutes les
        // branches de la projection sont ainsi exercées.
        let etapes: &[Statut] = match i % 5 {
            0 => &[Statut::Sale],
            1 => &[Statut::Sale, Statut::Propre],
            2 => &[Statut::Sale, Statut::Propre, Statut::Rempli],
            3 => &[Statut::Sale, Statut::Propre, Statut::Rempli, Statut::Livre],
            _ => &[
                Statut::Sale,
                Statut::Propre,
                Statut::Rempli,
                Statut::Livre,
                Statut::Recu,
            ],
        };
        for etape in etapes {
            let mut d =
                persistance::evenement::DemandeEvenement::nouvelle(*id, domaine::Origine::Document);
            d.statut_cible = Some(*etape);
            match etape {
                Statut::Rempli => {
                    d.ctx.type_biere_id = Some(biere);
                    d.ctx.lot_id = Some(lot);
                    d.ctx.cuve_id = Some(cuve);
                }
                Statut::Livre => {
                    d.ctx.client_id = Some(client);
                    d.ctx.adresse_client_id = Some(adresse);
                }
                _ => {}
            }
            persistance::evenement::enregistrer(&mut tx, Role::Administrateur, &d)
                .await
                .unwrap();
        }
    }
    tx.valider().await.unwrap();
}

#[tokio::test]
async fn rejouer_les_evenements_redonne_l_etat_courant() {
    let bac = Bac::nouveau().await;
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let admin = bac
        .creer_utilisateur(tenant, Role::Administrateur, "a@test.fr")
        .await;
    histoire_credible(&bac, tenant, admin, 50).await;

    let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
    let rapport = persistance::reconstruction::executer(tx.conn(), false)
        .await
        .unwrap();
    tx.annuler().await.unwrap();

    assert_eq!(rapport.futs_examines, 50);
    assert!(
        rapport.futs_diverges.is_empty(),
        "le rejeu diverge de la projection : {:#?}",
        rapport.futs_diverges.iter().take(3).collect::<Vec<_>>()
    );

    bac.detruire().await;
}

#[tokio::test]
async fn une_projection_corrompue_est_detectee_puis_corrigee() {
    let bac = Bac::nouveau().await;
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let admin = bac
        .creer_utilisateur(tenant, Role::Administrateur, "a@test.fr")
        .await;
    histoire_credible(&bac, tenant, admin, 10).await;

    // On simule un bug de projection : le cache ment, le flux dit vrai.
    let cible: Uuid = {
        let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
        let id: Uuid = sqlx::query_scalar(
            "select id from fut where statut <> 'perdu' order by code_qr limit 1",
        )
        .fetch_one(tx.conn())
        .await
        .unwrap();
        sqlx::query("update fut set statut = 'perdu', lot_id = null where id = $1")
            .bind(id)
            .execute(tx.conn())
            .await
            .unwrap();
        tx.valider().await.unwrap();
        id
    };

    // --verifier : détecte sans rien écrire.
    let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
    let detection = persistance::reconstruction::executer(tx.conn(), false)
        .await
        .unwrap();
    tx.annuler().await.unwrap();
    assert_eq!(
        detection.futs_diverges.len(),
        1,
        "la divergence doit être repérée"
    );
    assert_eq!(
        detection.futs_corriges, 0,
        "le mode vérification n'écrit rien"
    );
    assert!(detection.futs_diverges[0]
        .details
        .iter()
        .any(|d| d.contains("statut")));

    // Le fût est toujours faussement « perdu ».
    let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
    assert_eq!(
        persistance::fut::lire(tx.conn(), cible)
            .await
            .unwrap()
            .statut,
        Statut::Perdu
    );
    // Reconstruction réelle.
    let correction = persistance::reconstruction::executer(tx.conn(), true)
        .await
        .unwrap();
    assert_eq!(correction.futs_corriges, 1);
    tx.valider().await.unwrap();

    // Et plus aucune divergence.
    let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
    let f = persistance::fut::lire(tx.conn(), cible).await.unwrap();
    assert_ne!(
        f.statut,
        Statut::Perdu,
        "la projection doit avoir été remise d'aplomb"
    );
    let controle = persistance::reconstruction::executer(tx.conn(), false)
        .await
        .unwrap();
    assert!(controle.futs_diverges.is_empty());
    tx.annuler().await.unwrap();

    bac.detruire().await;
}
