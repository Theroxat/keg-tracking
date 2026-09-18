use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use maud::html;

use crate::i18n::message_erreur;
use crate::t;

#[derive(Debug)]
pub enum ErreurWeb {
    /// Redirection après action réussie (motif POST/Redirect/GET).
    Vers(String),
    NonAuthentifie,
    Verrouillee,
    Interdit,
    Introuvable,
    Csrf,
    TropDeRequetes,
    Metier(domaine::ErreurMetier),
    /// Erreurs de formulaire, par champ : `(nom_du_champ, message)`.
    Champs(Vec<(String, String)>),
    Conflit(String),
    Interne(String),
}

impl ErreurWeb {
    pub fn champ(nom: &str, message: impl Into<String>) -> ErreurWeb {
        ErreurWeb::Champs(vec![(nom.to_string(), message.into())])
    }

    pub fn code(&self) -> StatusCode {
        match self {
            ErreurWeb::Vers(_) => StatusCode::SEE_OTHER,
            ErreurWeb::NonAuthentifie | ErreurWeb::Verrouillee => StatusCode::SEE_OTHER,
            ErreurWeb::Interdit => StatusCode::FORBIDDEN,
            ErreurWeb::Introuvable => StatusCode::NOT_FOUND,
            ErreurWeb::Csrf => StatusCode::BAD_REQUEST,
            ErreurWeb::TropDeRequetes => StatusCode::TOO_MANY_REQUESTS,
            ErreurWeb::Metier(_) | ErreurWeb::Champs(_) | ErreurWeb::Conflit(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            ErreurWeb::Interne(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(&self) -> String {
        match self {
            ErreurWeb::Vers(_) => String::new(),
            ErreurWeb::NonAuthentifie => t!("connexion.session_expiree").into(),
            ErreurWeb::Verrouillee => t!("verrouillage.titre").into(),
            ErreurWeb::Interdit => t!("app.acces_refuse").into(),
            ErreurWeb::Introuvable => t!("app.page_introuvable").into(),
            ErreurWeb::Csrf => t!("erreur.csrf").into(),
            ErreurWeb::TropDeRequetes => t!("connexion.trop_de_tentatives").into(),
            ErreurWeb::Metier(e) => message_erreur(e),
            ErreurWeb::Conflit(_) => t!("erreur.deja_utilise").into(),
            ErreurWeb::Champs(c) => c
                .iter()
                .map(|(_, m)| m.clone())
                .collect::<Vec<_>>()
                .join(" "),
            ErreurWeb::Interne(_) => t!("app.erreur_serveur").into(),
        }
    }
}

impl IntoResponse for ErreurWeb {
    fn into_response(self) -> Response {
        match &self {
            ErreurWeb::Vers(url) => return Redirect::to(url).into_response(),
            ErreurWeb::NonAuthentifie => return Redirect::to("/connexion").into_response(),
            ErreurWeb::Verrouillee => return Redirect::to("/verrouillage").into_response(),
            ErreurWeb::Interne(detail) => {
                tracing::error!(erreur = %detail, "erreur interne");
            }
            _ => {}
        }
        let code = self.code();
        let corps = html! {
            div class="erreur-page" {
                p class="erreur-page__code" { (code.as_u16()) }
                p class="erreur-page__texte" { (self.message()) }
                a class="bouton" href="/" { (t!("nav.tableau_de_bord")) }
            }
        };
        (
            code,
            crate::gabarits::socle::page_nue(&self.message(), corps),
        )
            .into_response()
    }
}

impl From<persistance::Erreur> for ErreurWeb {
    fn from(e: persistance::Erreur) -> Self {
        match e {
            persistance::Erreur::Introuvable => ErreurWeb::Introuvable,
            persistance::Erreur::Metier(m) => ErreurWeb::Metier(m),
            persistance::Erreur::Conflit(c) => ErreurWeb::Conflit(c),
            autre => ErreurWeb::Interne(autre.to_string()),
        }
    }
}

impl From<domaine::ErreurMetier> for ErreurWeb {
    fn from(e: domaine::ErreurMetier) -> Self {
        ErreurWeb::Metier(e)
    }
}

impl From<sqlx::Error> for ErreurWeb {
    fn from(e: sqlx::Error) -> Self {
        ErreurWeb::Interne(e.to_string())
    }
}
