//! Isolation multi-locataire — le test le plus important du projet.
//!
//! Une fuite de données entre brasseries est un risque existentiel pour le
//! produit. Ce test tente délibérément ce qu'un bug ferait : une requête
//! sans filtre, une écriture chez le voisin. Il est écrit avant les écrans
//! et il ne doit jamais casser.

mod commun;

use commun::{tables_metier, Bac};
use sqlx::{Executor, Row};

#[tokio::test]
async fn aucune_table_metier_n_echappe_a_la_rls() {
    let bac = Bac::nouveau().await;
    let mut owner = bac.connexion_owner().await;
    let tables = tables_metier(&mut owner).await;
    assert!(tables.len() >= 12, "catalogue suspect : {tables:?}");

    for table in &tables {
        let ligne = sqlx::query(
            "select c.relrowsecurity,
                    (select count(*) from pg_policy p
                      where p.polrelid = c.oid and p.polname = 'isolation_locataire') as politiques
               from pg_class c join pg_namespace n on n.oid = c.relnamespace
              where n.nspname = 'public' and c.relname = $1",
        )
        .bind(table)
        .fetch_one(&mut owner)
        .await
        .unwrap();
        let rls: bool = ligne.get(0);
        let politiques: i64 = ligne.get(1);
        assert!(
            rls,
            "la table {table} porte tenant_id mais n'a pas la RLS activée"
        );
        assert_eq!(
            politiques, 1,
            "la table {table} n'a pas de politique d'isolation"
        );
    }

    // `locataire` n'a pas de colonne tenant_id : sa politique lui est propre,
    // on la vérifie séparément plutôt que de l'oublier.
    let rls: bool =
        sqlx::query_scalar("select relrowsecurity from pg_class where relname = 'locataire'")
            .fetch_one(&mut owner)
            .await
            .unwrap();
    assert!(rls, "la table locataire n'a pas la RLS activée");

    bac.detruire().await;
}

#[tokio::test]
async fn le_role_applicatif_n_est_pas_proprietaire_des_tables() {
    // C'est l'erreur classique d'implémentation de la RLS : un rôle
    // propriétaire contourne les politiques silencieusement.
    let bac = Bac::nouveau().await;
    let mut owner = bac.connexion_owner().await;
    let proprietaires: Vec<String> = sqlx::query_scalar(
        "select distinct pg_get_userbyid(c.relowner)
           from pg_class c join pg_namespace n on n.oid = c.relnamespace
          where n.nspname = 'public' and c.relkind = 'r'",
    )
    .fetch_all(&mut owner)
    .await
    .unwrap();
    assert!(
        !proprietaires.iter().any(|p| p == "webqr_app"),
        "webqr_app ne doit posséder aucune table : {proprietaires:?}"
    );

    for role in ["webqr_app", "webqr_auth", "webqr_editeur"] {
        let ligne = sqlx::query("select rolsuper, rolbypassrls from pg_roles where rolname = $1")
            .bind(role)
            .fetch_one(&mut owner)
            .await
            .unwrap();
        let superutilisateur: bool = ligne.get(0);
        let contourne: bool = ligne.get(1);
        assert!(
            !superutilisateur,
            "{role} ne doit pas être superutilisateur"
        );
        assert!(!contourne, "{role} ne doit pas porter BYPASSRLS");
    }
    bac.detruire().await;
}

#[tokio::test]
async fn une_requete_sans_filtre_ne_voit_que_son_locataire() {
    let bac = Bac::nouveau().await;
    let a = bac.creer_locataire("alpha", "ALPHA").await;
    let b = bac.creer_locataire("beta", "BETA").await;

    // Des données bien distinctes de chaque côté.
    for (tenant, marque) in [(a, "ALPHA"), (b, "BETA")] {
        let mut tx = bac.depot.ouvrir(tenant, None).await.unwrap();
        persistance::referentiel::creer_cuve(tx.conn(), &format!("Cuve {marque}"))
            .await
            .unwrap();
        persistance::referentiel::creer_biere(tx.conn(), &format!("Bière {marque}"), "#aabbcc", 0)
            .await
            .unwrap();
        persistance::client::creer(
            tx.conn(),
            &persistance::client::DonneesClient {
                societe: &format!("Client {marque}"),
                contact_nom: "",
                contact_prenom: "",
                telephone: "",
                email: "",
                commentaire: "",
            },
        )
        .await
        .unwrap();
        persistance::fut::creer_en_masse(
            tx.conn(),
            &[format!("{marque}00001"), format!("{marque}00002")],
            30,
            None,
            None,
        )
        .await
        .unwrap();
        tx.valider().await.unwrap();
    }

    let mut owner = bac.connexion_owner().await;
    let tables = tables_metier(&mut owner).await;
    let mut conn = bac.connexion_app().await;

    // --- 1. paramètre positionné sur A : aucune ligne de B, jamais.
    // `SET LOCAL` n'a d'effet que dans une transaction : c'est exactement
    // la façon dont la garde applicative l'emploie.
    conn.execute("begin").await.unwrap();
    conn.execute(format!("set local app.locataire_id = '{a}'").as_str())
        .await
        .unwrap();
    for table in &tables {
        // `select *` **sans aucun where** : c'est exactement ce qu'un
        // `where tenant_id = …` oublié produirait.
        let etrangeres: i64 = sqlx::query_scalar(&format!(
            "select count(*) from {table} where tenant_id <> '{a}'"
        ))
        .fetch_one(&mut conn)
        .await
        .unwrap_or_else(|e| panic!("lecture de {table} : {e}"));
        assert_eq!(
            etrangeres, 0,
            "fuite depuis {table} : {etrangeres} lignes d'un autre locataire"
        );
    }
    // Et il voit bien ses propres lignes : la politique n'est pas un simple
    // « tout bloquer », ce qui rendrait le test vide de sens.
    let miennes: i64 = sqlx::query_scalar("select count(*) from cuve")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(miennes, 1, "le locataire doit voir sa propre cuve");
    let futs: i64 = sqlx::query_scalar("select count(*) from fut")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(futs, 2, "le locataire doit voir ses propres fûts");
    conn.execute("commit").await.unwrap();

    // --- 2. aucun paramètre positionné : échec fermé, zéro ligne.
    let mut nu = bac.connexion_app().await;
    for table in &tables {
        let n: i64 = sqlx::query_scalar(&format!("select count(*) from {table}"))
            .fetch_one(&mut nu)
            .await
            .unwrap();
        assert_eq!(n, 0, "{table} est lisible sans locataire positionné");
    }

    bac.detruire().await;
}

#[tokio::test]
async fn ecrire_chez_le_voisin_est_refuse() {
    let bac = Bac::nouveau().await;
    let a = bac.creer_locataire("alpha", "ALPHA").await;
    let b = bac.creer_locataire("beta", "BETA").await;

    let mut tx = bac.depot.ouvrir(b, None).await.unwrap();
    persistance::referentiel::creer_cuve(tx.conn(), "Cuve de B")
        .await
        .unwrap();
    tx.valider().await.unwrap();

    // --- 3. INSERT portant le tenant_id du voisin.
    let mut conn = bac.connexion_app().await;
    conn.execute("begin").await.unwrap();
    conn.execute(format!("set local app.locataire_id = '{a}'").as_str())
        .await
        .unwrap();
    let insertion = sqlx::query("insert into cuve (tenant_id, nom) values ($1, 'Pirate')")
        .bind(b)
        .execute(&mut conn)
        .await;
    assert!(
        insertion.is_err(),
        "l'insertion chez un autre locataire doit être refusée"
    );
    conn.execute("rollback").await.unwrap();

    // --- 4. UPDATE déplaçant une ligne vers le voisin.
    let mut conn = bac.connexion_app().await;
    conn.execute("begin").await.unwrap();
    conn.execute(format!("set local app.locataire_id = '{a}'").as_str())
        .await
        .unwrap();
    sqlx::query("insert into cuve (tenant_id, nom) values ($1, 'Cuve de A')")
        .bind(a)
        .execute(&mut conn)
        .await
        .unwrap();
    let deplacement = sqlx::query("update cuve set tenant_id = $1")
        .bind(b)
        .execute(&mut conn)
        .await;
    assert!(
        deplacement.is_err(),
        "déplacer une ligne vers un autre locataire doit être refusé"
    );
    conn.execute("rollback").await.unwrap();

    // --- 5. la garde applicative laisse-t-elle fuir entre deux requêtes ?
    // `SET LOCAL` est annulé au commit, donc une connexion réutilisée depuis
    // le pool ne garde rien du locataire précédent.
    {
        let mut tx = bac.depot.ouvrir(a, None).await.unwrap();
        let _ = persistance::referentiel::lister_cuves(tx.conn(), false)
            .await
            .unwrap();
        tx.valider().await.unwrap();
    }
    let mut apres = bac.connexion_app().await;
    let residu: i64 = sqlx::query_scalar("select count(*) from cuve")
        .fetch_one(&mut apres)
        .await
        .unwrap();
    assert_eq!(
        residu, 0,
        "le locataire fuit d'une transaction à la suivante"
    );

    bac.detruire().await;
}

#[tokio::test]
async fn le_role_d_authentification_ne_voit_pas_le_metier() {
    // Le pool `auth` peut lire en travers des locataires : c'est nécessaire
    // pour résoudre un cookie. La contrepartie est qu'il ne doit rien
    // pouvoir lire d'autre.
    let bac = Bac::nouveau().await;
    let serveur = std::env::var("SERVEUR_TEST").unwrap_or_else(|_| "localhost:5432".into());
    let url = format!(
        "postgres://webqr_auth:{}@{}/{}",
        std::env::var("MDP_AUTH").unwrap(),
        serveur,
        bac.base
    );
    let mut conn = sqlx::PgConnection::connect(&url).await.unwrap();

    for table in [
        "fut",
        "evenement_fut",
        "client",
        "lot",
        "document_mouvement",
        "super_admin",
    ] {
        let r = sqlx::query(&format!("select count(*) from {table}"))
            .fetch_one(&mut conn)
            .await;
        assert!(r.is_err(), "webqr_auth ne doit pas pouvoir lire {table}");
    }
    // Il lit bien ce dont il a besoin.
    for table in ["utilisateur", "session", "locataire"] {
        let r = sqlx::query(&format!("select count(*) from {table}"))
            .fetch_one(&mut conn)
            .await;
        assert!(r.is_ok(), "webqr_auth doit pouvoir lire {table}");
    }
    // Mais il ne peut pas se promouvoir administrateur.
    let r = sqlx::query("update utilisateur set role = 'administrateur'")
        .execute(&mut conn)
        .await;
    assert!(
        r.is_err(),
        "webqr_auth ne doit pas pouvoir écrire la colonne role"
    );

    bac.detruire().await;
}

#[tokio::test]
async fn le_flux_d_evenements_est_en_ajout_seul() {
    let bac = Bac::nouveau().await;
    let a = bac.creer_locataire("alpha", "ALPHA").await;
    let mut tx = bac.depot.ouvrir(a, None).await.unwrap();
    persistance::fut::creer_en_masse(tx.conn(), &["ALPHA00001".into()], 30, None, None)
        .await
        .unwrap();
    tx.valider().await.unwrap();

    // Même le propriétaire des tables ne peut pas réécrire l'histoire :
    // le trigger vaut pour tous les rôles.
    let mut owner = bac.connexion_owner().await;
    let modif = sqlx::query("update evenement_fut set commentaire = 'falsifié'")
        .execute(&mut owner)
        .await;
    assert!(
        modif.is_err(),
        "un événement ne doit pas pouvoir être modifié"
    );
    let suppression = sqlx::query("delete from evenement_fut")
        .execute(&mut owner)
        .await;
    assert!(
        suppression.is_err(),
        "un événement ne doit pas pouvoir être supprimé"
    );

    bac.detruire().await;
}

use sqlx::Connection;
