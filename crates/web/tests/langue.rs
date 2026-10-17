//! Langue d'interface choisie à l'exécution.
//!
//! Ordre de résolution : préférence de l'utilisateur connecté, puis cookie
//! `wq_langue`, puis en-tête `Accept-Language`, enfin le français. Les
//! valeurs attendues sont lues dans les catalogues, pas recopiées ici.

mod commun;

use commun::{lancer, Bac, Serveur};
use domaine::Role;
use web::i18n::{en, fr};

fn valeur(catalogue: &[(&str, &'static str)], cle: &str) -> &'static str {
    catalogue
        .iter()
        .find(|(k, _)| *k == cle)
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("clé {cle} absente"))
}

/// Les apostrophes sont échappées par Maud ; on compare sur du texte
/// échappé de la même façon.
fn echappe(s: &str) -> String {
    maud::html! { (s) }.into_string()
}

async fn page(serveur: &Serveur, chemin: &str, en_tetes: &[(&str, &str)]) -> String {
    let mut r = serveur.client.get(format!("{}{chemin}", serveur.base));
    for (k, v) in en_tetes {
        r = r.header(*k, *v);
    }
    let rep = r.send().await.unwrap();
    assert_eq!(rep.status(), 200, "{chemin}");
    rep.text().await.unwrap()
}

#[tokio::test]
async fn la_page_de_connexion_suit_accept_language_puis_le_cookie() {
    let bac = Bac::nouveau().await;
    let serveur = lancer(&bac).await;
    let bouton_fr = echappe(valeur(fr::CATALOGUE, "connexion.bouton"));
    let bouton_en = echappe(valeur(en::CATALOGUE, "connexion.bouton"));
    assert_ne!(
        bouton_fr, bouton_en,
        "le catalogue anglais doit traduire ce bouton"
    );

    // Sans indication : français.
    let corps = page(&serveur, "/connexion", &[]).await;
    assert!(corps.contains(r#"<html lang="fr">"#));
    assert!(corps.contains(&bouton_fr));

    // Le navigateur préfère l'anglais.
    let corps = page(
        &serveur,
        "/connexion",
        &[("accept-language", "en-GB,en;q=0.9,fr;q=0.5")],
    )
    .await;
    assert!(corps.contains(r#"<html lang="en">"#), "{corps}");
    assert!(corps.contains(&bouton_en));
    assert!(!corps.contains(&bouton_fr));

    // Le cookie l'emporte sur l'en-tête.
    let corps = page(
        &serveur,
        "/connexion",
        &[("accept-language", "en"), ("cookie", "wq_langue=fr")],
    )
    .await;
    assert!(corps.contains(r#"<html lang="fr">"#));
    assert!(corps.contains(&bouton_fr));

    // Une valeur de cookie inconnue est ignorée, pas fatale.
    let corps = page(
        &serveur,
        "/connexion",
        &[("accept-language", "en"), ("cookie", "wq_langue=xx")],
    )
    .await;
    assert!(corps.contains(&bouton_en));

    bac.detruire().await;
}

#[tokio::test]
async fn le_lien_de_langue_pose_le_cookie_sans_redirection_ouverte() {
    let bac = Bac::nouveau().await;
    let serveur = lancer(&bac).await;

    let rep = serveur
        .client
        .get(format!("{}/langue/en?retour=%2Fconnexion", serveur.base))
        .send()
        .await
        .unwrap();
    assert_eq!(rep.status(), 303);
    assert_eq!(rep.headers()["location"], "/connexion");
    let cookie = rep.headers()["set-cookie"].to_str().unwrap();
    assert!(cookie.starts_with("wq_langue=en;"), "{cookie}");

    for retour in [
        "%2F%2Fexemple.org",
        "https%3A%2F%2Fexemple.org",
        "%2F%5Cexemple.org",
    ] {
        let rep = serveur
            .client
            .get(format!("{}/langue/fr?retour={retour}", serveur.base))
            .send()
            .await
            .unwrap();
        assert_eq!(rep.status(), 303);
        assert_eq!(rep.headers()["location"], "/", "retour {retour}");
    }

    let rep = serveur
        .client
        .get(format!("{}/langue/xx", serveur.base))
        .send()
        .await
        .unwrap();
    assert_eq!(rep.status(), 404);

    bac.detruire().await;
}

#[tokio::test]
async fn la_preference_enregistree_prime_sur_le_cookie() {
    let bac = Bac::nouveau().await;
    let tenant = bac.creer_locataire("alpha", "ALPHA").await;
    let u = bac
        .creer_utilisateur(tenant, Role::Lecture, "lecture@test.fr")
        .await;
    let (cookie, csrf) = Serveur::session(&bac, tenant, u).await;
    let serveur = lancer(&bac).await;
    let titre_fr = echappe(valeur(fr::CATALOGUE, "compte.titre"));
    let titre_en = echappe(valeur(en::CATALOGUE, "compte.titre"));

    let rep = serveur
        .client
        .post(format!("{}/compte/langue", serveur.base))
        .header("cookie", format!("wq_session={cookie}"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={csrf}&langue=en"))
        .send()
        .await
        .unwrap();
    assert_eq!(rep.status(), 303);
    assert!(rep
        .headers()
        .get_all("set-cookie")
        .iter()
        .any(|c| c.to_str().unwrap().starts_with("wq_langue=en;")));

    let langue: String = sqlx::query_scalar("select langue from utilisateur where id = $1")
        .bind(u)
        .fetch_one(&mut bac.connexion_owner().await)
        .await
        .unwrap();
    assert_eq!(langue, "en");

    // Le cookie et le navigateur disent français ; le compte dit anglais.
    let corps = page(
        &serveur,
        "/compte",
        &[
            ("cookie", &format!("wq_session={cookie}; wq_langue=fr")),
            ("accept-language", "fr"),
        ],
    )
    .await;
    assert!(corps.contains(r#"<html lang="en">"#));
    assert!(corps.contains(&titre_en));
    assert!(!corps.contains(&titre_fr));

    // Une langue non livrée est refusée sans rien écrire.
    let rep = serveur
        .client
        .post(format!("{}/compte/langue", serveur.base))
        .header("cookie", format!("wq_session={cookie}"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={csrf}&langue=de"))
        .send()
        .await
        .unwrap();
    assert_eq!(rep.status(), 422);

    bac.detruire().await;
}
