pub mod administration;
pub mod auth;
pub mod clients;
pub mod documents;
pub mod editeur;
pub mod etiquettes;
pub mod futs;
pub mod import;
pub mod lots;
pub mod parc;
pub mod sante;
pub mod scan;
pub mod tableau;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use domaine::Role;

use crate::gabarits::socle::Avis;
use crate::{middleware as inter, Etat};

/// Identifiant optionnel d'un formulaire : vide ou invalide donne `None`.
pub(crate) fn uuid_opt(v: &Option<String>) -> Option<uuid::Uuid> {
    v.as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
}

/// Adresse du client et avis en attente, extraits ensemble parce que toute
/// page en a besoin.
pub struct Contexte {
    pub ip: Option<String>,
    pub avis: Option<Avis>,
    pub chemin: String,
}

impl Contexte {
    pub fn avis_present(&self) -> bool {
        self.avis.is_some()
    }
}

impl FromRequestParts<Etat> for Contexte {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, etat: &Etat) -> Result<Self, Self::Rejection> {
        let ip = inter::adresse_depuis_parts(parts, etat.config.derriere_proxy).or_else(|| {
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|c| c.0.ip().to_string())
        });
        Ok(Contexte {
            ip,
            avis: inter::avis_depuis_cookie(parts),
            chemin: parts.uri.path().to_string(),
        })
    }
}

/// Redirection après action, avec un message affiché sur la page d'arrivée.
pub fn rediriger(url: &str, ton: &str, texte: &str, securise: bool) -> Response {
    (
        axum::http::StatusCode::SEE_OTHER,
        [
            (axum::http::header::LOCATION, url.to_string()),
            (
                axum::http::header::SET_COOKIE,
                inter::cookie_avis(ton, texte, securise),
            ),
        ],
    )
        .into_response()
}

pub fn rediriger_nu(url: &str) -> Response {
    axum::response::Redirect::to(url).into_response()
}

/// Enveloppe une page pour effacer l'avis une fois affiché.
pub fn page_avec_avis(corps: maud::Markup, avait_avis: bool, securise: bool) -> Response {
    if avait_avis {
        (
            [(
                axum::http::header::SET_COOKIE,
                inter::cookie_avis_efface(securise),
            )],
            corps,
        )
            .into_response()
    } else {
        corps.into_response()
    }
}

/// Inventaire des routes protégées, avec le rôle minimal exigé.
///
/// Sert de source unique au test de contrôle d'accès : chaque entrée est
/// appelée avec chacun des trois rôles, et doit répondre 403 quand le rôle
/// est insuffisant. Un test compare aussi le nombre de routes déclarées
/// dans ce fichier à la taille de cet inventaire, pour qu'une route ajoutée
/// sans rôle ne passe pas inaperçue.
pub const INVENTAIRE: &[(&str, &str, Role)] = &[
    ("GET", "/", Role::Lecture),
    ("GET", "/compte", Role::Lecture),
    ("POST", "/compte/mot-de-passe", Role::Lecture),
    ("POST", "/compte/pin", Role::Lecture),
    ("POST", "/compte/sessions/revoquer", Role::Lecture),
    ("GET", "/parc", Role::Lecture),
    ("GET", "/parc/export", Role::Lecture),
    ("GET", "/futs/{id}", Role::Lecture),
    ("GET", "/clients/adresses", Role::Lecture),
    ("POST", "/futs/{id}/transition", Role::Production),
    ("POST", "/futs/{id}/identite", Role::Production),
    ("GET", "/clients", Role::Lecture),
    ("GET", "/clients/{id}", Role::Lecture),
    ("GET", "/documents", Role::Lecture),
    ("GET", "/documents/{id}", Role::Lecture),
    ("GET", "/documents/{id}/bon-de-livraison", Role::Lecture),
    ("GET", "/clients/nouveau", Role::Production),
    ("POST", "/clients", Role::Production),
    ("POST", "/clients/{id}", Role::Production),
    ("POST", "/clients/{id}/archiver", Role::Production),
    ("POST", "/clients/{id}/adresses", Role::Production),
    ("POST", "/clients/{id}/adresses/{aid}", Role::Production),
    ("GET", "/scan", Role::Production),
    ("POST", "/api/scan", Role::Production),
    ("GET", "/operations/nouvelle", Role::Production),
    ("POST", "/operations", Role::Production),
    ("GET", "/operations/{id}", Role::Production),
    ("GET", "/operations/{id}/lignes", Role::Production),
    ("POST", "/api/operations/{id}/lignes", Role::Production),
    (
        "POST",
        "/operations/{id}/lignes/{lid}/retirer",
        Role::Production,
    ),
    ("POST", "/operations/{id}/vider", Role::Production),
    ("POST", "/operations/{id}/abandonner", Role::Production),
    ("GET", "/operations/{id}/valider", Role::Production),
    ("POST", "/operations/{id}/valider", Role::Production),
    ("GET", "/lots", Role::Lecture),
    ("GET", "/lots/{id}/rappel", Role::Lecture),
    ("GET", "/lots/{id}/rappel/export", Role::Lecture),
    ("POST", "/lots", Role::Production),
    ("GET", "/administration", Role::Administrateur),
    ("GET", "/administration/referentiels", Role::Administrateur),
    (
        "POST",
        "/administration/referentiels/{genre}",
        Role::Administrateur,
    ),
    (
        "POST",
        "/administration/referentiels/{genre}/{id}",
        Role::Administrateur,
    ),
    ("GET", "/administration/utilisateurs", Role::Administrateur),
    ("POST", "/administration/utilisateurs", Role::Administrateur),
    (
        "GET",
        "/administration/utilisateurs/{id}",
        Role::Administrateur,
    ),
    (
        "POST",
        "/administration/utilisateurs/{id}",
        Role::Administrateur,
    ),
    ("GET", "/administration/parametres", Role::Administrateur),
    ("POST", "/administration/parametres", Role::Administrateur),
    ("GET", "/administration/journal", Role::Administrateur),
    ("GET", "/administration/etiquettes", Role::Administrateur),
    (
        "GET",
        "/administration/etiquettes/planche",
        Role::Administrateur,
    ),
    ("POST", "/administration/etiquettes", Role::Administrateur),
    ("GET", "/administration/import", Role::Administrateur),
    ("POST", "/administration/import", Role::Administrateur),
];

pub fn routeur(etat: Etat) -> Router {
    let publiques = Router::new()
        .route(
            "/connexion",
            get(auth::afficher_connexion).post(auth::traiter_connexion),
        )
        .route(
            "/verrouillage",
            get(auth::afficher_verrouillage).post(auth::traiter_pin),
        )
        .route("/deconnexion", post(auth::deconnexion));

    let lecture = Router::new()
        .route("/", get(tableau::afficher))
        .route("/compte", get(auth::afficher_compte))
        .route("/compte/mot-de-passe", post(auth::changer_mot_de_passe))
        .route("/compte/pin", post(auth::definir_pin))
        .route("/compte/sessions/revoquer", post(auth::revoquer_session))
        .route("/parc", get(parc::afficher))
        .route("/parc/export", get(parc::exporter))
        .route("/futs/{id}", get(futs::fiche))
        .route("/clients/adresses", get(futs::adresses_du_client))
        .route("/clients", get(clients::liste))
        .route("/clients/{id}", get(clients::fiche))
        .route("/documents", get(documents::liste))
        .route("/documents/{id}", get(documents::fiche))
        .route(
            "/documents/{id}/bon-de-livraison",
            get(documents::bon_de_livraison),
        )
        .route("/lots", get(lots::liste))
        .route("/lots/{id}/rappel", get(lots::rappel))
        .route("/lots/{id}/rappel/export", get(lots::rappel_export))
        .layer(from_fn_with_state(Role::Lecture, inter::garde_role));

    let production = Router::new()
        .route("/futs/{id}/transition", post(futs::transition))
        .route("/futs/{id}/identite", post(futs::identite))
        .route("/clients/nouveau", get(clients::formulaire_nouveau))
        .route("/clients", post(clients::creer))
        .route("/clients/{id}", post(clients::modifier))
        .route("/clients/{id}/archiver", post(clients::archiver))
        .route("/clients/{id}/adresses", post(clients::ajouter_adresse))
        .route(
            "/clients/{id}/adresses/{aid}",
            post(clients::modifier_adresse),
        )
        .route("/scan", get(scan::afficher))
        .route("/api/scan", post(scan::resoudre))
        .route("/operations/nouvelle", get(documents::nouvelle))
        .route("/operations", post(documents::creer))
        .route("/operations/{id}", get(documents::capture))
        .route("/operations/{id}/lignes", get(documents::fragment_captures))
        .route("/api/operations/{id}/lignes", post(documents::capturer))
        .route(
            "/operations/{id}/lignes/{lid}/retirer",
            post(documents::retirer_ligne),
        )
        .route("/operations/{id}/vider", post(documents::vider))
        .route("/operations/{id}/abandonner", post(documents::abandonner))
        .route(
            "/operations/{id}/valider",
            get(documents::recapitulatif).post(documents::valider),
        )
        .route("/lots", post(lots::creer))
        .layer(from_fn_with_state(Role::Production, inter::garde_role));

    let administration = Router::new()
        .route("/administration", get(administration::accueil))
        .route(
            "/administration/referentiels",
            get(administration::referentiels),
        )
        .route(
            "/administration/referentiels/{genre}",
            post(administration::creer_referentiel),
        )
        .route(
            "/administration/referentiels/{genre}/{id}",
            post(administration::modifier_referentiel),
        )
        .route(
            "/administration/utilisateurs",
            get(administration::utilisateurs).post(administration::creer_utilisateur),
        )
        .route(
            "/administration/utilisateurs/{id}",
            get(administration::fiche_utilisateur).post(administration::modifier_utilisateur),
        )
        .route(
            "/administration/parametres",
            get(administration::parametres).post(administration::enregistrer_parametres),
        )
        .route("/administration/journal", get(administration::journal))
        .route(
            "/administration/etiquettes",
            get(etiquettes::afficher).post(etiquettes::creer),
        )
        .route(
            "/administration/etiquettes/planche",
            get(etiquettes::planche),
        )
        .route(
            "/administration/import",
            get(import::afficher).post(import::traiter),
        )
        .layer(from_fn_with_state(Role::Administrateur, inter::garde_role));

    // Périmètre éditeur : sa propre session, son propre cookie, aucune
    // intersection avec les sessions de locataires.
    let editeur = Router::new()
        .route(
            "/editeur/connexion",
            get(editeur::connexion_get).post(editeur::connexion_post),
        )
        .route("/editeur", get(editeur::accueil))
        .route("/editeur/deconnexion", post(editeur::deconnexion))
        .route("/editeur/brasseries", post(editeur::creer_brasserie))
        .route("/editeur/brasseries/{id}/etat", post(editeur::changer_etat))
        .route(
            "/editeur/brasseries/{id}/assistance",
            post(editeur::assistance),
        )
        .route("/editeur/journal", get(editeur::journal))
        .layer(from_fn_with_state(etat.clone(), editeur::session_editeur));

    let assistance = Router::new().route(
        "/editeur/assistance/arreter",
        post(editeur::arreter_assistance),
    );

    let statiques = Router::new()
        .route("/statique/app.css", get(crate::statique::css))
        .route("/statique/htmx.js", get(crate::statique::htmx))
        .route("/statique/scan.js", get(crate::statique::scan_js))
        .route("/statique/zxing-reader.js", get(crate::statique::zxing_js))
        .route(
            "/statique/zxing_reader.wasm",
            get(crate::statique::zxing_wasm),
        );

    let technique = Router::new()
        .route("/sante", get(sante::sante))
        .route("/pret", get(sante::pret));

    Router::new()
        .merge(publiques)
        .merge(lecture)
        .merge(production)
        .merge(administration)
        .merge(editeur)
        .merge(assistance)
        .merge(statiques)
        .merge(technique)
        .fallback(sante::introuvable)
        .layer(from_fn_with_state(etat.clone(), inter::csrf))
        .layer(from_fn_with_state(etat.clone(), inter::session_courante))
        .layer(from_fn(inter::en_tetes_securite))
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(
            tower_http::trace::TraceLayer::new_for_http().make_span_with(
                tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::INFO),
            ),
        )
        .with_state(etat)
}
