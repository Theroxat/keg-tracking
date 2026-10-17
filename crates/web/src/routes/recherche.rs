use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use maud::html;
use serde::Deserialize;

use crate::gabarits::composants as c;
use crate::gabarits::socle::{page, Cadre};
use crate::middleware::Session;
use crate::routes::{page_avec_avis, rediriger_nu, Contexte};
use crate::{t, ErreurWeb, Etat};

#[derive(Deserialize, Default)]
pub struct Requete {
    q: Option<String>,
}

pub async fn afficher(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Query(r): Query<Requete>,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let saisie = r.q.unwrap_or_default().trim().to_string();
    let resultats = if saisie.is_empty() {
        None
    } else {
        let mut tx = etat
            .depot
            .ouvrir(s.tenant_id, Some(s.utilisateur_id))
            .await?;
        let res = persistance::recherche::chercher(tx.conn(), &saisie).await?;
        tx.valider().await?;
        Some(res)
    };

    // Un seul résultat, identique à la saisie : on va droit à la fiche.
    if let Some(res) = &resultats {
        if res.total() == 1 && res.exacts == 1 {
            if let Some(f) = res.futs.first() {
                return Ok(rediriger_nu(&format!("/futs/{}", f.id)));
            }
            if let Some(cl) = res.clients.first() {
                return Ok(rediriger_nu(&format!("/clients/{}", cl.id)));
            }
            if let Some(l) = res.lots.first() {
                return Ok(rediriger_nu(&format!("/lots/{}", l.id)));
            }
        }
    }

    let corps = html! {
        @match &resultats {
            None => { (c::vide(t!("recherche.invite"), None)) }
            Some(res) if res.total() == 0 => {
                (c::vide(&t!("recherche.aucun", terme = &saisie), None))
            }
            Some(res) => {
                @if !res.futs.is_empty() {
                    section class="carte" style="margin-bottom:14px" {
                        div class="carte__tete" { h2 { (t!("recherche.futs")) } }
                        div class="carte__corps" { ul class="liste-simple" {
                            @for f in &res.futs {
                                li { a href=(format!("/futs/{}", f.id)) { (f.code_qr) } }
                            }
                        } }
                    }
                }
                @if !res.clients.is_empty() {
                    section class="carte" style="margin-bottom:14px" {
                        div class="carte__tete" { h2 { (t!("recherche.clients")) } }
                        div class="carte__corps" { ul class="liste-simple" {
                            @for cl in &res.clients {
                                li { a href=(format!("/clients/{}", cl.id)) { (cl.societe) } }
                            }
                        } }
                    }
                }
                @if !res.lots.is_empty() {
                    section class="carte" style="margin-bottom:14px" {
                        div class="carte__tete" { h2 { (t!("recherche.lots")) } }
                        div class="carte__corps" { ul class="liste-simple" {
                            @for l in &res.lots {
                                li { a href=(format!("/lots/{}", l.id)) { (l.numero) } }
                            }
                        } }
                    }
                }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("recherche.titre")).avis(ctx.avis);
    Ok(page_avec_avis(page(cadre, corps), avait, etat.config.cookies_securises).into_response())
}
