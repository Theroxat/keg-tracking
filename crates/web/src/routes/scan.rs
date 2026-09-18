use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use maud::{html, Markup, PreEscaped};
use serde::{Deserialize, Serialize};

use crate::gabarits::socle::{page, Cadre};
use crate::i18n::traduire;
use crate::middleware::Session;
use crate::routes::{page_avec_avis, Contexte};
use crate::statique;
use crate::{t, ErreurWeb, Etat};

/// Réponse consommée par l'îlot de scan. Une seule forme pour les deux
/// modes (unitaire et capture), pour que le JavaScript reste trivial.
#[derive(Serialize)]
pub struct Reponse {
    pub resultat: &'static str,
    pub code: String,
    pub titre: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rediriger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rafraichir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compteur: Option<i64>,
}

#[derive(Deserialize)]
pub struct Demande {
    pub code_qr: String,
}

/// Bloc commun aux deux écrans de scan : viseur, champ douchette, retour
/// plein écran, saisie manuelle, journal des derniers scans.
pub fn ilot(
    url_envoi: &str,
    cible_rafraichir: Option<&str>,
    jeton_csrf: &str,
    camera: bool,
) -> Markup {
    let v = statique::version();
    html! {
        div id="scan"
            data-url=(url_envoi)
            data-csrf=(jeton_csrf)
            data-relance="900"
            data-camera=(if camera { "oui" } else { "non" })
            data-rafraichir=[cible_rafraichir]
            data-zxing-js={ "/statique/zxing-reader.js?v=" (v) }
            data-zxing-wasm={ "/statique/zxing_reader.wasm?v=" (v) }
            data-libelle-natif=(t!("scan.decodeur_natif"))
            data-libelle-wasm=(t!("scan.decodeur_wasm"))
            data-libelle-camera-indisponible=(t!("scan.camera_indisponible"))
            data-libelle-camera-refusee=(t!("scan.camera_refusee"))
            data-libelle-contexte-non-securise=(t!("scan.contexte_non_securise"))
            data-libelle-erreur-reseau=(t!("app.erreur_serveur")) {

            // Champ invisible qui garde le focus : la douchette Bluetooth
            // tape le code comme un clavier et trouve toujours sa cible.
            input id="champ-douchette" type="text" autocomplete="off"
                  inputmode="none" aria-hidden="true" tabindex="-1";

            div class="viseur" {
                video id="scan-video" playsinline muted autoplay {}
                div class="viseur__cadre" {}
                div id="scan-hors" class="viseur__hors rien" {}
                div class="viseur__outils" {
                    button type="button" id="scan-basculer" class="bouton bouton--petit" {
                        (t!("scan.basculer_camera"))
                    }
                    button type="button" id="scan-torche" class="bouton bouton--petit rien"
                            aria-pressed="false" {
                        (t!("scan.torche"))
                    }
                }
            }

            div style="display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin-top:10px" {
                span class="minus" id="scan-decodeur" { (t!("app.chargement")) }
                span id="douchette-temoin" class="douchette-temoin rien" {
                    span class="pastille" style="background:var(--ok)" {}
                    (t!("scan.douchette_active"))
                }
            }

            form id="scan-manuel" style="display:flex;gap:8px;margin-top:10px" {
                input type="text" name="code_qr" placeholder=(t!("scan.saisie_placeholder"))
                      autocomplete="off" autocapitalize="characters" spellcheck="false";
                button type="submit" class="bouton" { (t!("action.valider")) }
            }

            div id="scan-retour" class="retour" aria-live="assertive" {}
        }
        script src={ "/statique/scan.js?v=" (v) } defer {}
        (PreEscaped(""))
    }
}

pub async fn afficher(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let corps = html! {
        div class="scan" {
            div { (ilot("/api/scan", None, &s.jeton_csrf, true)) }
            aside class="carte" {
                div class="carte__tete" { h2 { (t!("scan.derniers_scans")) } }
                div class="carte__corps carte__corps--serre" {
                    ul class="journal-scan" id="scan-journal" {
                        li data-vide="1" class="discret petit" { (t!("scan.aucun_scan")) }
                    }
                }
            }
        }
    };
    let avait = ctx.avis_present();
    let cadre = Cadre::new(&s, &ctx.chemin, t!("scan.titre"))
        .sous_titre(t!("scan.attente"))
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

/// Scan unitaire : le fût est retrouvé, jamais créé. Un code inconnu est
/// une erreur explicite — étiquette non enregistrée, fût d'une autre
/// brasserie, ou code abîmé — et le scanner reprend tout seul.
pub async fn resoudre(
    State(etat): State<Etat>,
    Session(s): Session,
    Json(d): Json<Demande>,
) -> Response {
    let cle = format!("scan:{}", s.utilisateur_id);
    if !etat
        .debit
        .autoriser(&cle, 240, std::time::Duration::from_secs(60))
    {
        return Json(Reponse {
            resultat: "echec",
            code: d.code_qr.clone(),
            titre: t!("connexion.trop_de_tentatives").into(),
            detail: None,
            rediriger: None,
            rafraichir: None,
            compteur: None,
        })
        .into_response();
    }

    let mut tx = match etat.depot.ouvrir(s.tenant_id, Some(s.utilisateur_id)).await {
        Ok(t) => t,
        Err(e) => return erreur_json(&d.code_qr, e.to_string()),
    };
    let resultat = persistance::fut::lire_par_code(tx.conn(), &d.code_qr).await;
    let _ = tx.valider().await;

    match resultat {
        Ok(f) => Json(Reponse {
            resultat: "succes",
            code: f.code_qr.clone(),
            titre: f.code_qr.clone(),
            detail: Some(format!(
                "{} · {} L{}",
                traduire(f.statut.cle_i18n()),
                f.contenance_litres,
                f.client.map(|c| format!(" · {c}")).unwrap_or_default()
            )),
            rediriger: Some(format!("/futs/{}", f.id)),
            rafraichir: None,
            compteur: None,
        })
        .into_response(),
        Err(persistance::Erreur::Metier(m)) => Json(Reponse {
            resultat: "echec",
            code: d.code_qr.clone(),
            titre: t!("scan.echec").into(),
            detail: Some(crate::i18n::message_erreur(&m)),
            rediriger: None,
            rafraichir: None,
            compteur: None,
        })
        .into_response(),
        Err(e) => erreur_json(&d.code_qr, e.to_string()),
    }
}

fn erreur_json(code: &str, detail: String) -> Response {
    tracing::warn!(erreur = %detail, "scan");
    Json(Reponse {
        resultat: "echec",
        code: code.to_string(),
        titre: t!("app.erreur_serveur").into(),
        detail: None,
        rediriger: None,
        rafraichir: None,
        compteur: None,
    })
    .into_response()
}
