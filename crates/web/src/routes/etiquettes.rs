//! Génération d'étiquettes QR et mise en service en masse.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::Form;
use maud::{html, PreEscaped};
use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};
use serde::Deserialize;

use crate::gabarits::composants as c;
use crate::gabarits::csrf;
use crate::gabarits::socle::{page, page_nue, Cadre};
use crate::middleware::Session;
use crate::routes::{page_avec_avis, rediriger, uuid_opt, Contexte};
use crate::{t, ErreurWeb, Etat};

#[derive(Deserialize, Default)]
pub struct Requete {
    #[serde(default)]
    de: Option<i64>,
    #[serde(default)]
    a: Option<i64>,
}

fn codes(prefixe: &str, de: i64, a: i64) -> Vec<String> {
    (de..=a).map(|n| format!("{prefixe}{n:05}")).collect()
}

pub async fn afficher(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Query(r): Query<Requete>,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    let emplacements = persistance::referentiel::lister_emplacements(tx.conn(), true).await?;
    let dernier: Option<String> =
        sqlx::query_scalar("select code_qr from fut order by code_qr desc limit 1")
            .fetch_optional(tx.conn())
            .await?;
    let suivant = dernier
        .as_deref()
        .and_then(|c| c.strip_prefix(&loc.prefixe_qr))
        .and_then(|n| n.parse::<i64>().ok())
        .map(|n| n + 1)
        .unwrap_or(1);
    let de = r.de.unwrap_or(suivant).max(1);
    let a = r.a.unwrap_or(de + 99).max(de);
    // Au-delà de la limite de création, l'aperçu n'interroge pas la base.
    let deja_pris = if a - de <= 5000 {
        persistance::fut::codes_deja_pris(tx.conn(), &codes(&loc.prefixe_qr, de, a))
            .await?
            .len() as i64
    } else {
        0
    };
    tx.valider().await?;
    let nombre = a - de + 1;
    let a_creer = nombre - deja_pris;
    let premier = format!("{}{:05}", loc.prefixe_qr, de);
    let dernier_code = format!("{}{:05}", loc.prefixe_qr, a);
    let confirmation = t!(
        "etiq.creer_confirmation",
        n = a_creer,
        premier = &premier,
        dernier = &dernier_code
    );

    let corps = html! {
        (c::retour_administration())
        div class="avis" { (t!("etiq.conseil_materiel")) }
        div class="colonnes" {
            section class="carte" {
                div class="carte__tete" { h2 { (t!("etiq.generer")) } }
                div class="carte__corps" {
                    form method="get" action="/administration/etiquettes" {
                        div class="grille" {
                            (c::Champ::texte("de", t!("etiq.de"), de.to_string()).de_type("number").rendre())
                            (c::Champ::texte("a", t!("etiq.a"), a.to_string()).de_type("number").rendre())
                        }
                        p class="champ__aide" {
                            (t!("etiq.apercu", premier = &premier, dernier = &dernier_code))
                            " — " (t!("etiq.nb_etiquettes", n = nombre))
                        }
                        p class="champ__aide" {
                            (t!("etiq.bilan", creer = a_creer, existants = deja_pris))
                        }
                        button class="bouton" { (t!("etiq.apercu_plage")) }
                    }
                    hr;
                    form method="post" action="/administration/etiquettes"
                          onsubmit=(c::confirmer_soumission(&confirmation)) {
                        (csrf(&s.jeton_csrf))
                        input type="hidden" name="de" value=(de);
                        input type="hidden" name="a" value=(a);
                        (c::selection("emplacement_id", t!("champ.emplacement"),
                            &c::options(&emplacements, |x| x.id, |x| x.nom.clone()),
                            loc.emplacement_defaut_id.map(|i| i.to_string()).as_deref(), Some("—"), false))
                        (c::selection("contenance_litres", t!("champ.contenance"),
                            &loc.contenances_autorisees.iter().map(|l| (l.to_string(), format!("{l} {}", t!("unite.litres")))).collect::<Vec<_>>(),
                            None, None, true))
                        button class="bouton bouton--vif bouton--large" { (t!("etiq.creer_futs")) }
                    }
                }
            }
            section class="carte" {
                div class="carte__tete" { h2 { (t!("etiq.planche")) } }
                div class="carte__corps" {
                    p class="petit discret" { (t!("etiq.planche_note")) }
                    a class="bouton bouton--vif bouton--large" target="_blank"
                      href=(format!("/administration/etiquettes/planche?de={de}&a={a}")) {
                        (t!("etiq.planche"))
                    }
                }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("etiq.titre"))
        .sous_titre(format!("{} : {}", t!("etiq.prefixe"), loc.prefixe_qr))
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

/// Planche imprimable calée sur un format d'étiquettes du commerce.
/// Une page HTML avec une feuille d'impression soignée suffit, et ne coûte
/// aucune dépendance : la génération PDF viendra si le besoin est réel.
pub async fn planche(
    State(etat): State<Etat>,
    Session(s): Session,
    Query(r): Query<Requete>,
) -> Result<Response, ErreurWeb> {
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    tx.valider().await?;
    let de = r.de.unwrap_or(1).max(1);
    let a = r.a.unwrap_or(de + 23).max(de).min(de + 999);

    let corps = html! {
        div class="sans-impression" style="padding:14px;display:flex;gap:10px;align-items:center" {
            button class="bouton bouton--vif" data-imprimer { (t!("app.imprimer")) }
            span class="petit discret" { (t!("etiq.planche_note")) }
        }
        div class="planche" {
            @for code in codes(&loc.prefixe_qr, de, a) {
                div class="etiquette" {
                    (PreEscaped(qr_svg(&code)))
                    div {
                        div class="etiquette__code" { (code) }
                        div class="etiquette__marque" { (loc.nom) }
                    }
                }
            }
        }
    };
    Ok(page_nue(t!("etiq.planche"), corps).into_response())
}

fn qr_svg(donnee: &str) -> String {
    // Correction d'erreur élevée : l'étiquette vivra dans un atelier humide,
    // et un code partiellement abîmé doit rester lisible.
    match QrCode::with_error_correction_level(donnee.as_bytes(), EcLevel::H) {
        Ok(code) => code
            .render::<svg::Color>()
            .min_dimensions(160, 160)
            .quiet_zone(true)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build(),
        Err(_) => String::new(),
    }
}

#[derive(Deserialize)]
pub struct FormCreation {
    de: i64,
    a: i64,
    contenance_litres: i32,
    #[serde(default)]
    emplacement_id: Option<String>,
}

pub async fn creer(
    State(etat): State<Etat>,
    Session(s): Session,
    Form(f): Form<FormCreation>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let retour = "/administration/etiquettes";
    if f.a < f.de || f.a - f.de > 5000 {
        return Ok(rediriger(
            retour,
            "grave",
            t!("erreur.nombre_invalide"),
            securise,
        ));
    }

    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    domaine::contenance::verifier(f.contenance_litres, &loc.contenances_autorisees)?;

    let liste = codes(&loc.prefixe_qr, f.de, f.a);
    // Refus avant écriture : une plage qui chevauche l'existant est une
    // erreur de saisie, pas quelque chose à ignorer silencieusement.
    let pris = persistance::fut::codes_deja_pris(tx.conn(), &liste).await?;
    if !pris.is_empty() {
        let apercu: Vec<String> = pris.iter().take(6).cloned().collect();
        let suite = if pris.len() > 6 {
            format!(" … (+{})", pris.len() - 6)
        } else {
            String::new()
        };
        return Ok(rediriger(
            retour,
            "grave",
            &t!(
                "etiq.conflit",
                codes = format!("{}{}", apercu.join(", "), suite)
            ),
            securise,
        ));
    }

    let n = persistance::fut::creer_en_masse(
        tx.conn(),
        &liste,
        f.contenance_litres,
        uuid_opt(&f.emplacement_id).or(loc.emplacement_defaut_id),
        Some(s.utilisateur_id),
    )
    .await?;
    persistance::audit::tracer(
        tx.conn(),
        Some(s.utilisateur_id),
        "futs.mise_en_service_masse",
        Some("fut"),
        None,
        serde_json::json!({ "de": f.de, "a": f.a, "nombre": n }),
        None,
    )
    .await?;
    tx.valider().await?;
    Ok(rediriger(
        &format!("/administration/etiquettes/planche?de={}&a={}", f.de, f.a),
        "ok",
        &t!("etiq.crees", n = n),
        securise,
    ))
}
