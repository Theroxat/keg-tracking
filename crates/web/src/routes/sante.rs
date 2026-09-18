use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::{ErreurWeb, Etat};

/// Sonde de vivacité : répond tant que le processus tourne.
pub async fn sante() -> Response {
    (StatusCode::OK, "ok").into_response()
}

/// Sonde de disponibilité : vérifie aussi que la base répond.
pub async fn pret(State(etat): State<Etat>) -> Response {
    if etat.depot.en_bonne_sante().await {
        (StatusCode::OK, "pret").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "base indisponible").into_response()
    }
}

pub async fn introuvable() -> Response {
    ErreurWeb::Introuvable.into_response()
}
