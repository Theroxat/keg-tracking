//! Back-office éditeur : hors du périmètre RLS des locataires.
//!
//! Il n'a que trois responsabilités : créer une brasserie et son premier
//! administrateur, suspendre ou réactiver un locataire, consulter l'état du
//! système. L'inscription est provisionnée manuellement en v1 — pas de
//! libre-service.

use axum::extract::{Path, State};
use axum::http::{header, request::Parts, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::{extract::FromRequestParts, Form};
use domaine::Role;
use maud::{html, Markup};
use persistance::locataire::EtatLocataire;
use persistance::super_admin::{self, ContexteEditeur};
use serde::Deserialize;
use uuid::Uuid;

use crate::gabarits::composants as c;
use crate::gabarits::csrf;
use crate::gabarits::socle::page_nue;
use crate::routes::Contexte;
use crate::{t, ErreurWeb, Etat};

pub struct Editeur(pub ContexteEditeur);

impl<S: Send + Sync> FromRequestParts<S> for Editeur {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ContexteEditeur>()
            .cloned()
            .map(Editeur)
            .ok_or_else(|| Redirect::to("/editeur/connexion").into_response())
    }
}

fn cookie(jeton: &str, securise: bool) -> String {
    format!(
        "{}={}; Path=/editeur; Max-Age={}; HttpOnly; SameSite=Lax{}",
        super_admin::NOM_COOKIE,
        jeton,
        super_admin::DUREE_SESSION_HEURES * 3600,
        if securise { "; Secure" } else { "" }
    )
}

fn cadre(titre: &str, e: Option<&ContexteEditeur>, corps: Markup) -> Markup {
    page_nue(
        titre,
        html! {
            div class="appli" style="grid-template-columns:1fr" {
                div class="corps" {
                    header class="entete" {
                        div class="entete__titre" {
                            h1 { (t!("editeur.titre")) }
                            p { (titre) }
                        }
                        @if let Some(e) = e {
                            div class="boutons" {
                                a class="bouton bouton--petit" href="/editeur" { (t!("editeur.brasseries")) }
                                a class="bouton bouton--petit" href="/editeur/journal" { (t!("editeur.journal")) }
                                form method="post" action="/editeur/deconnexion" {
                                    (csrf(&e.jeton_csrf))
                                    button class="bouton bouton--petit bouton--nu" { (t!("nav.deconnexion")) }
                                }
                            }
                        }
                    }
                    main class="contenu" { (corps) }
                }
            }
        },
    )
}

// ------------------------------------------------------------------ connexion
pub async fn afficher_connexion(erreur: Option<&str>) -> Markup {
    page_nue(
        t!("editeur.connexion"),
        html! {
            div class="portail" {
                form class="portail__carte" method="post" action="/editeur/connexion" {
                    div class="portail__marque" {
                        b { (t!("editeur.titre")) }
                        span { (t!("editeur.connexion")) }
                    }
                    @if let Some(e) = erreur { div class="avis avis--grave" { (e) } }
                    (c::Champ::texte("email", t!("champ.email"), "").de_type("email").requis().rendre())
                    (c::Champ::texte("mot_de_passe", t!("champ.mot_de_passe"), "")
                        .de_type("password").requis().rendre())
                    button class="bouton bouton--vif bouton--large" { (t!("connexion.bouton")) }
                }
            }
        },
    )
}

pub async fn connexion_get() -> Response {
    afficher_connexion(None).await.into_response()
}

#[derive(Deserialize)]
pub struct FormConnexion {
    email: String,
    mot_de_passe: String,
}

pub async fn connexion_post(
    State(etat): State<Etat>,
    ctx: Contexte,
    Form(f): Form<FormConnexion>,
) -> Response {
    let cle = format!("editeur:{}", ctx.ip.as_deref().unwrap_or("?"));
    if !etat
        .debit
        .autoriser(&cle, 8, std::time::Duration::from_secs(60))
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            afficher_connexion(Some(t!("connexion.trop_de_tentatives"))).await,
        )
            .into_response();
    }
    let trouve = super_admin::chercher_pour_connexion(
        etat.depot.auth_editeur(),
        &f.email.trim().to_lowercase(),
    )
    .await;
    let Ok(Some((id, hachage, actif))) = trouve else {
        let _ = etat.hacheur.verifier(&f.mot_de_passe, "$argon2id$v=19$m=19456,t=2,p=1$c2VsMTIzNDU2Nzg$0000000000000000000000000000000000000000000");
        return (
            StatusCode::UNAUTHORIZED,
            afficher_connexion(Some(t!("connexion.echec"))).await,
        )
            .into_response();
    };
    if !actif || !etat.hacheur.verifier(&f.mot_de_passe, &hachage) {
        return (
            StatusCode::UNAUTHORIZED,
            afficher_connexion(Some(t!("connexion.echec"))).await,
        )
            .into_response();
    }
    let jeton =
        match super_admin::ouvrir_session(etat.depot.auth_editeur(), id, ctx.ip.as_deref()).await {
            Ok(j) => j,
            Err(e) => return ErreurWeb::Interne(e.to_string()).into_response(),
        };
    if let Ok(mut tx) = etat.depot.ouvrir_editeur().await {
        let _ = super_admin::tracer(
            tx.conn(),
            Some(id),
            "editeur.connexion",
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
            (header::LOCATION, "/editeur".to_string()),
            (
                header::SET_COOKIE,
                cookie(&jeton, etat.config.cookies_securises),
            ),
        ],
    )
        .into_response()
}

pub async fn deconnexion(State(etat): State<Etat>, Editeur(e): Editeur) -> Response {
    let _ = super_admin::revoquer_session(etat.depot.auth_editeur(), e.session_id).await;
    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/editeur/connexion".to_string()),
            (
                header::SET_COOKIE,
                format!(
                    "{}=; Path=/editeur; Max-Age=0; HttpOnly; SameSite=Lax{}",
                    super_admin::NOM_COOKIE,
                    if etat.config.cookies_securises {
                        "; Secure"
                    } else {
                        ""
                    }
                ),
            ),
        ],
    )
        .into_response()
}

// ------------------------------------------------------------------ accueil
pub async fn accueil(State(etat): State<Etat>, Editeur(e): Editeur) -> Result<Response, ErreurWeb> {
    let mut tx = etat.depot.ouvrir_editeur().await?;
    let locataires = persistance::locataire::lister(tx.conn()).await?;
    let systeme = super_admin::etat_systeme(tx.conn()).await?;
    tx.valider().await?;

    let corps = html! {
        div class="cartes" style="margin-bottom:14px" {
            div class="tuile" { span class="tuile__valeur num" { (systeme.locataires_actifs) }
                                span class="tuile__nom" { "Brasseries actives" } }
            div class="tuile" { span class="tuile__valeur num" { (systeme.utilisateurs) }
                                span class="tuile__nom" { (t!("util.titre")) } }
            div class="tuile" { span class="tuile__valeur num" { (systeme.futs) }
                                span class="tuile__nom" { "Fûts suivis" } }
            div class="tuile" { span class="tuile__valeur num" { (systeme.evenements) }
                                span class="tuile__nom" { "Événements" } }
        }
        div class="tableau-cadre" {
            table class="tableau" {
                thead { tr {
                    th { (t!("champ.nom")) } th { "Identifiant" } th { (t!("etiq.prefixe")) }
                    th { (t!("champ.statut")) } th { "Créée" } th {}
                } }
                tbody {
                    @for l in &locataires {
                        tr {
                            td style="font-weight:600" { (l.nom) }
                            td class="mono petit" { (l.identifiant_court) }
                            td class="mono petit" { (l.prefixe_qr) }
                            td {
                                @if l.etat == EtatLocataire::Actif { (c::badge("ok", t!("locataire.actif"))) }
                                @else { (c::badge("grave", t!("locataire.suspendu"))) }
                            }
                            td class="petit discret num" { (c::date(l.cree_le, &l.fuseau_horaire)) }
                            td class="serre boutons" {
                                form method="post" action=(format!("/editeur/brasseries/{}/etat", l.id)) {
                                    (csrf(&e.jeton_csrf))
                                    input type="hidden" name="etat"
                                          value=(if l.etat == EtatLocataire::Actif { "suspendu" } else { "actif" });
                                    button class="bouton bouton--petit" {
                                        @if l.etat == EtatLocataire::Actif { (t!("editeur.suspendre")) }
                                        @else { (t!("editeur.reactiver")) }
                                    }
                                }
                                form method="post" action=(format!("/editeur/brasseries/{}/assistance", l.id)) {
                                    (csrf(&e.jeton_csrf))
                                    button class="bouton bouton--petit bouton--grave" {
                                        (t!("editeur.usurpation"))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        section class="carte" style="margin-top:16px;max-width:640px" {
            div class="carte__tete" { h2 { (t!("editeur.nouvelle_brasserie")) } }
            div class="carte__corps" {
                form method="post" action="/editeur/brasseries" {
                    (csrf(&e.jeton_csrf))
                    div class="grille" {
                        (c::Champ::texte("nom", t!("champ.nom"), "").requis().rendre())
                        (c::Champ::texte("slug", "Identifiant court", "")
                            .requis().aide("minuscules, chiffres et tirets").rendre())
                        (c::Champ::texte("prefixe_qr", t!("etiq.prefixe"), "")
                            .requis().aide("majuscules, unique sur l'installation").rendre())
                        (c::Champ::texte("admin_email", t!("champ.email"), "")
                            .de_type("email").requis().rendre())
                        (c::Champ::texte("admin_prenom", t!("champ.prenom"), "").rendre())
                        (c::Champ::texte("admin_nom", t!("champ.nom"), "Administrateur").requis().rendre())
                    }
                    p class="champ__aide" { (t!("util.mot_de_passe_temporaire_note")) }
                    button class="bouton bouton--vif" { (t!("action.creer")) }
                }
            }
        }
    };
    Ok(cadre(t!("editeur.brasseries"), Some(&e), corps).into_response())
}

#[derive(Deserialize)]
pub struct FormBrasserie {
    nom: String,
    slug: String,
    prefixe_qr: String,
    admin_email: String,
    #[serde(default)]
    admin_prenom: String,
    admin_nom: String,
}

pub async fn creer_brasserie(
    State(etat): State<Etat>,
    ctx: Contexte,
    Editeur(e): Editeur,
    Form(f): Form<FormBrasserie>,
) -> Result<Response, ErreurWeb> {
    let mot_de_passe = mot_de_passe_genere();
    let hachage = etat
        .hacheur
        .hacher(&mot_de_passe)
        .map_err(|x| ErreurWeb::Interne(x.to_string()))?;

    let mut tx = etat.depot.ouvrir_editeur().await?;
    let locataire_id = persistance::locataire::creer(
        tx.conn(),
        &persistance::locataire::NouveauLocataire {
            nom: f.nom.trim().to_string(),
            identifiant_court: f.slug.trim().to_lowercase(),
            prefixe_qr: f.prefixe_qr.trim().to_uppercase(),
            fuseau_horaire: etat.config.fuseau_defaut.clone(),
        },
    )
    .await
    .map_err(|_| ErreurWeb::Conflit("slug ou préfixe QR déjà pris".into()))?;

    persistance::utilisateur::creer(
        tx.conn(),
        &persistance::utilisateur::NouvelUtilisateur {
            tenant_id: locataire_id,
            email: &f.admin_email.trim().to_lowercase(),
            mot_de_passe_hash: &hachage,
            nom: &f.admin_nom,
            prenom: &f.admin_prenom,
            role: Role::Administrateur,
        },
    )
    .await
    .map_err(|_| ErreurWeb::Conflit("adresse e-mail déjà utilisée".into()))?;

    sqlx::query(
        "insert into emplacement (tenant_id, nom)
         values ($1,'Brasserie'),($1,'Zone de lavage'),($1,'Quai')",
    )
    .bind(locataire_id)
    .execute(tx.conn())
    .await?;
    sqlx::query(
        "update locataire set emplacement_defaut_id =
           (select id from emplacement where tenant_id = $1 and nom = 'Brasserie')
         where id = $1",
    )
    .bind(locataire_id)
    .execute(tx.conn())
    .await?;

    super_admin::tracer(
        tx.conn(),
        Some(e.super_admin_id),
        "brasserie.creee",
        Some(locataire_id),
        serde_json::json!({ "slug": f.slug, "admin": f.admin_email }),
        ctx.ip.as_deref(),
    )
    .await?;
    tx.valider().await?;

    let corps = html! {
        div class="avis avis--ok" { (t!("editeur.creee")) }
        section class="carte" style="max-width:560px" {
            div class="carte__corps" {
                p { b { (f.nom) } " — " (f.slug.to_lowercase()) }
                p { (t!("champ.email")) " : " b { (f.admin_email) } }
                p { (t!("util.mot_de_passe_temporaire")) " : "
                    code style="font-size:16px" { (mot_de_passe) } }
                p class="petit discret" { (t!("util.mot_de_passe_temporaire_note")) }
                a class="bouton bouton--vif" href="/editeur" { (t!("app.retour")) }
            }
        }
    };
    Ok(cadre(t!("editeur.creee"), Some(&e), corps).into_response())
}

fn mot_de_passe_genere() -> String {
    use rand::Rng;
    const MOTS: &[&str] = &[
        "houblon", "levure", "brassin", "malte", "cuve", "saison", "ambree", "blonde", "garde",
        "tirage", "drache", "moulin",
    ];
    let mut rng = rand::thread_rng();
    let m: Vec<&str> = (0..4).map(|_| MOTS[rng.gen_range(0..MOTS.len())]).collect();
    format!("{}-{}!A", m.join("-"), rng.gen_range(100..999))
}

#[derive(Deserialize)]
pub struct FormEtat {
    etat: String,
}

pub async fn changer_etat(
    State(etat): State<Etat>,
    ctx: Contexte,
    Editeur(e): Editeur,
    Path(id): Path<Uuid>,
    Form(f): Form<FormEtat>,
) -> Result<Response, ErreurWeb> {
    let nouvel = if f.etat == "suspendu" {
        EtatLocataire::Suspendu
    } else {
        EtatLocataire::Actif
    };
    let mut tx = etat.depot.ouvrir_editeur().await?;
    persistance::locataire::changer_etat(tx.conn(), id, nouvel).await?;
    super_admin::tracer(
        tx.conn(),
        Some(e.super_admin_id),
        if nouvel == EtatLocataire::Suspendu {
            "brasserie.suspendue"
        } else {
            "brasserie.reactivee"
        },
        Some(id),
        serde_json::json!({}),
        ctx.ip.as_deref(),
    )
    .await?;
    tx.valider().await?;
    Ok(Redirect::to("/editeur").into_response())
}

/// Assistance : ouvre une session locataire marquée comme usurpée.
///
/// Elle est courte, journalisée à l'ouverture et à la fermeture, et
/// l'application affiche un bandeau permanent tant qu'elle dure.
pub async fn assistance(
    State(etat): State<Etat>,
    ctx: Contexte,
    Editeur(e): Editeur,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    let mut tx = etat.depot.ouvrir_editeur().await?;
    let admin: Option<(Uuid,)> = sqlx::query_as(
        "select id from utilisateur
          where tenant_id = $1 and role = 'administrateur' and actif
          order by cree_le limit 1",
    )
    .bind(id)
    .fetch_optional(tx.conn())
    .await?;
    let Some((utilisateur_id,)) = admin else {
        tx.valider().await?;
        return Err(ErreurWeb::Introuvable);
    };
    super_admin::tracer(
        tx.conn(),
        Some(e.super_admin_id),
        "assistance.ouverte",
        Some(id),
        serde_json::json!({ "utilisateur": utilisateur_id }),
        ctx.ip.as_deref(),
    )
    .await?;
    tx.valider().await?;

    let jeton = persistance::session::ouvrir_avec(
        etat.depot.auth(),
        id,
        utilisateur_id,
        Some("assistance éditeur"),
        ctx.ip.as_deref(),
        Some(e.super_admin_id),
        1,
    )
    .await?;
    super_admin::usurper(etat.depot.auth_editeur(), e.session_id, Some(id)).await?;

    Ok((
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/".to_string()),
            (
                header::SET_COOKIE,
                format!(
                    "{}={}; Path=/; Max-Age=3600; HttpOnly; SameSite=Lax{}",
                    persistance::session::NOM_COOKIE,
                    jeton,
                    if etat.config.cookies_securises {
                        "; Secure"
                    } else {
                        ""
                    }
                ),
            ),
        ],
    )
        .into_response())
}

pub async fn arreter_assistance(
    State(etat): State<Etat>,
    ctx: Contexte,
    crate::middleware::SessionBrute(s): crate::middleware::SessionBrute,
) -> Result<Response, ErreurWeb> {
    let _ = persistance::session::revoquer(etat.depot.auth(), s.session_id).await;
    if let (Some(par), Ok(mut tx)) = (s.usurpee_par, etat.depot.ouvrir_editeur().await) {
        let _ = super_admin::tracer(
            tx.conn(),
            Some(par),
            "assistance.fermee",
            Some(s.tenant_id),
            serde_json::json!({}),
            ctx.ip.as_deref(),
        )
        .await;
        let _ = tx.valider().await;
    }
    Ok((
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, "/editeur".to_string()),
            (
                header::SET_COOKIE,
                format!(
                    "{}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax{}",
                    persistance::session::NOM_COOKIE,
                    if etat.config.cookies_securises {
                        "; Secure"
                    } else {
                        ""
                    }
                ),
            ),
        ],
    )
        .into_response())
}

pub async fn journal(State(etat): State<Etat>, Editeur(e): Editeur) -> Result<Response, ErreurWeb> {
    let mut tx = etat.depot.ouvrir_editeur().await?;
    let entrees = super_admin::journal(tx.conn(), 300).await?;
    tx.valider().await?;
    let corps = html! {
        div class="tableau-cadre" {
            table class="tableau" {
                thead { tr {
                    th { (t!("journal.quand")) } th { (t!("journal.qui")) }
                    th { (t!("journal.action")) } th { "Brasserie" } th { "Détails" }
                } }
                tbody {
                    @for x in &entrees {
                        tr {
                            td class="petit num discret" { (c::date_heure(x.survenu_le, "Europe/Paris")) }
                            td class="petit" { (c::texte_ou_tiret(x.qui.as_ref())) }
                            td { (c::badge("neutre", &x.action)) }
                            td class="petit" { (c::texte_ou_tiret(x.locataire.as_ref())) }
                            td class="petit discret" { (x.details.to_string()) }
                        }
                    }
                }
            }
        }
    };
    Ok(cadre(t!("editeur.journal"), Some(&e), corps).into_response())
}

/// Intergiciel : résout la session éditeur sur les routes /editeur.
pub async fn session_editeur(
    State(etat): State<Etat>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let jeton = req
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|p| {
            let (k, val) = p.split_once('=')?;
            (k.trim() == super_admin::NOM_COOKIE).then(|| val.trim().to_string())
        })
        .next();
    if let Some(j) = jeton {
        if let Ok(Some(ctx)) = super_admin::resoudre_session(etat.depot.auth_editeur(), &j).await {
            req.extensions_mut().insert(ctx);
        }
    }
    next.run(req).await
}
