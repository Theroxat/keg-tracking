use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use maud::{html, Markup};
use persistance::authentification::{mot_de_passe_acceptable, pin_acceptable};
use persistance::locataire::EtatLocataire;
use persistance::session::{self, ContexteSession};
use serde::Deserialize;
use std::time::Duration;

use crate::gabarits::socle::{page, page_nue, Avis, Cadre};
use crate::gabarits::{composants as c, csrf};
use crate::i18n::traduire;
use crate::middleware::{Session, SessionBrute};
use crate::routes::{page_avec_avis, rediriger, rediriger_nu, Contexte};
use crate::{t, ErreurWeb, Etat};

const MAX_TENTATIVES_IP: usize = 12;
const FENETRE_IP: Duration = Duration::from_secs(60);

fn cookie_session(jeton: &str, securise: bool) -> String {
    format!(
        "{}={}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax{}",
        session::NOM_COOKIE,
        jeton,
        session::DUREE_SESSION_JOURS * 86_400,
        if securise { "; Secure" } else { "" }
    )
}

fn cookie_session_efface(securise: bool) -> String {
    format!(
        "{}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax{}",
        session::NOM_COOKIE,
        if securise { "; Secure" } else { "" }
    )
}

// ------------------------------------------------------------------ connexion
fn page_connexion(erreur: Option<&str>, email: &str) -> Markup {
    page_nue(
        t!("connexion.titre"),
        html! {
            div class="portail" {
                form class="portail__carte" method="post" action="/connexion" {
                    div class="portail__marque" {
                        b { (t!("app.nom")) }
                        span { (t!("connexion.sous_titre")) }
                    }
                    @if let Some(e) = erreur {
                        div class="avis avis--grave" role="alert" { (e) }
                    }
                    (c::Champ::texte("email", t!("champ.email"), email)
                        .de_type("email").requis().rendre())
                    (c::Champ::texte("mot_de_passe", t!("champ.mot_de_passe"), "")
                        .de_type("password").requis().rendre())
                    button type="submit" class="bouton bouton--vif bouton--large bouton--atelier" {
                        (t!("connexion.bouton"))
                    }
                    p class="petit discret" style="margin-top:14px;margin-bottom:0" {
                        (t!("connexion.se_souvenir"))
                    }
                }
            }
        },
    )
}

pub async fn afficher_connexion(ctx: Contexte) -> Response {
    let avis = ctx.avis.as_ref().map(|a| a.texte.clone());
    page_avec_avis(
        page_connexion(avis.as_deref(), ""),
        ctx.avis.is_some(),
        true,
    )
}

#[derive(Deserialize)]
pub struct FormConnexion {
    email: String,
    mot_de_passe: String,
}

pub async fn traiter_connexion(
    State(etat): State<Etat>,
    ctx: Contexte,
    Form(f): Form<FormConnexion>,
) -> Response {
    let cle = format!("connexion:{}", ctx.ip.as_deref().unwrap_or("inconnue"));
    if !etat.debit.autoriser(&cle, MAX_TENTATIVES_IP, FENETRE_IP) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            page_connexion(Some(t!("connexion.trop_de_tentatives")), &f.email),
        )
            .into_response();
    }

    let email = f.email.trim().to_lowercase();
    let candidat = match session::chercher_pour_connexion(etat.depot.auth(), &email).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(erreur = %e, "recherche de compte");
            return ErreurWeb::Interne(e.to_string()).into_response();
        }
    };

    let Some(candidat) = candidat else {
        // Compte inexistant : on hache quand même, pour ne pas révéler par
        // le temps de réponse quelles adresses existent.
        let _ = etat.hacheur.verifier(&f.mot_de_passe, "$argon2id$v=19$m=19456,t=2,p=1$c2VsMTIzNDU2Nzg$0000000000000000000000000000000000000000000");
        return (
            StatusCode::UNAUTHORIZED,
            page_connexion(Some(t!("connexion.echec")), &f.email),
        )
            .into_response();
    };

    if let Some(jusqu_a) = candidat.verrouille_jusqu_a {
        if jusqu_a > chrono::Utc::now() {
            let minutes = (jusqu_a - chrono::Utc::now()).num_minutes().max(1);
            return (
                StatusCode::TOO_MANY_REQUESTS,
                page_connexion(
                    Some(&t!("connexion.compte_verrouille", minutes = minutes)),
                    &f.email,
                ),
            )
                .into_response();
        }
    }

    if !etat
        .hacheur
        .verifier(&f.mot_de_passe, &candidat.mot_de_passe_hash)
    {
        let _ = session::enregistrer_echec(etat.depot.auth(), candidat.id).await;
        return (
            StatusCode::UNAUTHORIZED,
            page_connexion(Some(t!("connexion.echec")), &f.email),
        )
            .into_response();
    }
    if !candidat.actif {
        return (
            StatusCode::FORBIDDEN,
            page_connexion(Some(t!("connexion.compte_inactif")), &f.email),
        )
            .into_response();
    }
    if candidat.locataire_etat == EtatLocataire::Suspendu {
        return (
            StatusCode::FORBIDDEN,
            page_connexion(Some(t!("connexion.brasserie_suspendue")), &f.email),
        )
            .into_response();
    }

    let _ = session::reinitialiser_echecs(etat.depot.auth(), candidat.id).await;
    let jeton = match session::ouvrir(
        etat.depot.auth(),
        candidat.tenant_id,
        candidat.id,
        None,
        ctx.ip.as_deref(),
    )
    .await
    {
        Ok(j) => j,
        Err(e) => return ErreurWeb::Interne(e.to_string()).into_response(),
    };

    if let Ok(mut tx) = etat
        .depot
        .ouvrir(candidat.tenant_id, Some(candidat.id))
        .await
    {
        let _ = persistance::audit::tracer(
            tx.conn(),
            Some(candidat.id),
            "connexion",
            None,
            None,
            serde_json::json!({}),
            ctx.ip.as_deref(),
        )
        .await;
        let _ = tx.valider().await;
    }

    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/".to_string()),
            (
                header::SET_COOKIE,
                cookie_session(&jeton, etat.config.cookies_securises),
            ),
        ],
    )
        .into_response()
}

pub async fn deconnexion(State(etat): State<Etat>, SessionBrute(s): SessionBrute) -> Response {
    let _ = session::revoquer(etat.depot.auth(), s.session_id).await;
    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/connexion".to_string()),
            (
                header::SET_COOKIE,
                cookie_session_efface(etat.config.cookies_securises),
            ),
        ],
    )
        .into_response()
}

// ------------------------------------------------------------------ verrouillage
fn page_verrouillage(s: &ContexteSession, erreur: Option<&str>) -> Markup {
    page_nue(
        t!("verrouillage.titre"),
        html! {
            div class="portail" {
                form class="portail__carte" method="post" action="/verrouillage" {
                    (csrf(&s.jeton_csrf))
                    div class="portail__marque" {
                        b { (s.nom_complet()) }
                        span { (s.locataire_nom) }
                    }
                    p class="petit" { (t!("verrouillage.consigne")) }
                    @if let Some(e) = erreur { div class="avis avis--grave" role="alert" { (e) } }
                    @if s.a_un_pin {
                        div class="champ" {
                            label for="champ-pin" { (t!("champ.pin")) }
                            input id="champ-pin" name="pin" type="text" inputmode="numeric"
                                  pattern="[0-9]{6}" maxlength="6" autocomplete="off"
                                  autofocus required
                                  style="font-size:28px;letter-spacing:.4em;text-align:center;min-height:60px";
                        }
                        button type="submit" class="bouton bouton--vif bouton--large bouton--atelier" {
                            (t!("verrouillage.bouton"))
                        }
                    } @else {
                        div class="avis avis--alerte" { (t!("verrouillage.pas_de_pin")) }
                    }
                    div style="margin-top:16px;text-align:center" {
                        form method="post" action="/deconnexion" {
                            (csrf(&s.jeton_csrf))
                            button type="submit" class="bouton bouton--nu bouton--petit" {
                                (t!("verrouillage.autre_compte"))
                            }
                        }
                    }
                }
            }
        },
    )
}

pub async fn afficher_verrouillage(SessionBrute(s): SessionBrute) -> Response {
    if !s.verrouillee {
        return rediriger_nu("/");
    }
    page_verrouillage(&s, None).into_response()
}

#[derive(Deserialize)]
pub struct FormPin {
    pin: String,
}

pub async fn traiter_pin(
    State(etat): State<Etat>,
    ctx: Contexte,
    SessionBrute(s): SessionBrute,
    Form(f): Form<FormPin>,
) -> Response {
    let cle = format!("pin:{}", s.session_id);
    if !etat.debit.autoriser(&cle, 8, FENETRE_IP) {
        return page_verrouillage(&s, Some(t!("verrouillage.patienter"))).into_response();
    }
    if let Ok(Some(bloque)) = session::pin_bloque_jusqu_a(etat.depot.auth(), s.session_id).await {
        if bloque > chrono::Utc::now() {
            return page_verrouillage(&s, Some(t!("verrouillage.patienter"))).into_response();
        }
    }

    let hachage = match session::hachage_pin(etat.depot.auth(), s.utilisateur_id).await {
        Ok(Some(h)) => h,
        _ => return page_verrouillage(&s, Some(t!("verrouillage.pas_de_pin"))).into_response(),
    };

    if etat.hacheur.verifier(f.pin.trim(), &hachage) {
        let _ = session::deverrouiller(etat.depot.auth(), s.session_id).await;
        return rediriger_nu("/");
    }

    // Le PIN ne remplace jamais le mot de passe : au bout de cinq erreurs,
    // la session tombe et il faut se reconnecter complètement.
    match session::echec_pin(etat.depot.auth(), s.session_id).await {
        Ok(true) => rediriger(
            "/connexion",
            "grave",
            t!("verrouillage.revoquee"),
            etat.config.cookies_securises,
        ),
        _ => {
            let _ = ctx;
            page_verrouillage(&s, Some(t!("verrouillage.echec"))).into_response()
        }
    }
}

// ------------------------------------------------------------------ compte
pub async fn afficher_compte(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let sessions = session::lister_actives(etat.depot.auth(), s.utilisateur_id).await?;
    let avait = ctx.avis.is_some();
    let corps = html! {
        div class="cartes" style="grid-template-columns:repeat(auto-fit,minmax(300px,1fr))" {
            section class="carte" {
                div class="carte__tete" { h2 { (t!("compte.changer_mot_de_passe")) } }
                div class="carte__corps" {
                    form method="post" action="/compte/mot-de-passe" {
                        (csrf(&s.jeton_csrf))
                        (c::Champ::texte("actuel", t!("compte.mot_de_passe_actuel"), "")
                            .de_type("password").requis().rendre())
                        (c::Champ::texte("nouveau", t!("compte.nouveau_mot_de_passe"), "")
                            .de_type("password").requis()
                            .aide(t!("mdp.trop_court")).rendre())
                        button type="submit" class="bouton bouton--vif" { (t!("action.enregistrer")) }
                    }
                }
            }
            section class="carte" {
                div class="carte__tete" { h2 { (t!("compte.definir_pin")) } }
                div class="carte__corps" {
                    p class="petit discret" { (t!("compte.pin_explication")) }
                    form method="post" action="/compte/pin" {
                        (csrf(&s.jeton_csrf))
                        (c::Champ::texte("mot_de_passe", t!("compte.mot_de_passe_actuel"), "")
                            .de_type("password").requis().rendre())
                        (c::Champ::texte("pin", t!("champ.pin"), "")
                            .aide("6 chiffres").rendre())
                        div class="boutons" {
                            button type="submit" class="bouton bouton--vif" { (t!("action.enregistrer")) }
                            button type="submit" name="supprimer" value="1" class="bouton" {
                                (t!("action.supprimer"))
                            }
                        }
                    }
                }
            }
        }
        section class="carte" style="margin-top:12px" {
            div class="carte__tete" { h2 { (t!("compte.sessions")) } }
            div class="carte__corps carte__corps--serre" {
                div class="tableau-cadre" style="border:0" {
                    table class="tableau" {
                        thead { tr {
                            th { "Appareil" } th { (t!("champ.derniere_activite")) }
                            th { "Expire" } th {}
                        } }
                        tbody {
                            @for sess in &sessions {
                                tr {
                                    td {
                                        @if sess.id == s.session_id {
                                            (c::badge("ok", t!("compte.session_courante")))
                                        } @else {
                                            (c::texte_ou_tiret(sess.appareil_libelle.as_ref()))
                                        }
                                    }
                                    td class="num" { (c::depuis(sess.derniere_activite_le)) }
                                    td class="num petit discret" {
                                        (c::date(sess.expire_le, &s.fuseau_horaire))
                                    }
                                    td class="serre" {
                                        @if sess.id != s.session_id {
                                            form method="post" action="/compte/sessions/revoquer" {
                                                (csrf(&s.jeton_csrf))
                                                input type="hidden" name="session_id" value=(sess.id);
                                                button class="bouton bouton--petit bouton--grave" {
                                                    (t!("compte.revoquer_session"))
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("compte.titre"))
        .sous_titre(format!("{} — {}", s.email, traduire(s.role.cle_i18n())))
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct FormMotDePasse {
    actuel: String,
    nouveau: String,
}

pub async fn changer_mot_de_passe(
    State(etat): State<Etat>,
    Session(s): Session,
    Form(f): Form<FormMotDePasse>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let actuel =
        persistance::utilisateur::hachage_mot_de_passe(tx.conn(), s.utilisateur_id).await?;
    if !etat.hacheur.verifier(&f.actuel, &actuel) {
        return Ok(rediriger("/compte", "grave", t!("mdp.incorrect"), securise));
    }
    if let Err(cle) = mot_de_passe_acceptable(&f.nouveau) {
        return Ok(rediriger("/compte", "grave", traduire(cle), securise));
    }
    let hachage = etat
        .hacheur
        .hacher(&f.nouveau)
        .map_err(|e| ErreurWeb::Interne(e.to_string()))?;
    persistance::utilisateur::changer_mot_de_passe(tx.conn(), s.utilisateur_id, &hachage).await?;
    persistance::audit::tracer(
        tx.conn(),
        Some(s.utilisateur_id),
        "mot_de_passe.change",
        Some("utilisateur"),
        Some(&s.utilisateur_id.to_string()),
        serde_json::json!({}),
        None,
    )
    .await?;
    tx.valider().await?;

    // Un changement de mot de passe ferme les autres sessions.
    let _ = session::revoquer_utilisateur(etat.depot.auth(), s.utilisateur_id).await;
    let jeton =
        session::ouvrir(etat.depot.auth(), s.tenant_id, s.utilisateur_id, None, None).await?;
    Ok((
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/compte".to_string()),
            (header::SET_COOKIE, cookie_session(&jeton, securise)),
            (
                header::SET_COOKIE,
                crate::middleware::cookie_avis("ok", t!("compte.mot_de_passe_change"), securise),
            ),
        ],
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct FormDefinirPin {
    mot_de_passe: String,
    pin: Option<String>,
    supprimer: Option<String>,
}

pub async fn definir_pin(
    State(etat): State<Etat>,
    Session(s): Session,
    Form(f): Form<FormDefinirPin>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let actuel =
        persistance::utilisateur::hachage_mot_de_passe(tx.conn(), s.utilisateur_id).await?;
    if !etat.hacheur.verifier(&f.mot_de_passe, &actuel) {
        return Ok(rediriger("/compte", "grave", t!("mdp.incorrect"), securise));
    }
    if f.supprimer.is_some() {
        persistance::utilisateur::definir_pin(tx.conn(), s.utilisateur_id, None).await?;
        tx.valider().await?;
        return Ok(rediriger(
            "/compte",
            "ok",
            t!("compte.pin_supprime"),
            securise,
        ));
    }
    let pin = f.pin.unwrap_or_default();
    if let Err(cle) = pin_acceptable(pin.trim()) {
        return Ok(rediriger("/compte", "grave", traduire(cle), securise));
    }
    let hachage = etat
        .hacheur
        .hacher(pin.trim())
        .map_err(|e| ErreurWeb::Interne(e.to_string()))?;
    persistance::utilisateur::definir_pin(tx.conn(), s.utilisateur_id, Some(&hachage)).await?;
    tx.valider().await?;
    Ok(rediriger(
        "/compte",
        "ok",
        t!("compte.pin_defini"),
        securise,
    ))
}

#[derive(Deserialize)]
pub struct FormRevoquer {
    session_id: uuid::Uuid,
}

pub async fn revoquer_session(
    State(etat): State<Etat>,
    Session(s): Session,
    Form(f): Form<FormRevoquer>,
) -> Result<Response, ErreurWeb> {
    if f.session_id != s.session_id {
        session::revoquer(etat.depot.auth(), f.session_id).await?;
    }
    Ok(rediriger_nu("/compte"))
}

/// Utilisé par les écrans qui veulent afficher un avis une seule fois.
pub fn avis_ok(texte: &str) -> Avis {
    Avis::ok(texte)
}
