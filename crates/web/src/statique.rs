//! Ressources statiques embarquées dans le binaire.
//!
//! Aucun CDN : la politique de sécurité de contenu est stricte, et une
//! brasserie n'a pas à dépendre d'un tiers pour que son atelier fonctionne.
//! Tout est servi depuis notre domaine, et l'image Docker est autosuffisante.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

pub const CSS: &str = include_str!("../../../assets/css/app.css");
pub const HTMX: &str = include_str!("../../../assets/vendor/htmx.min.js");
pub const SCAN_JS: &str = include_str!("../../../assets/js/scan/scan.js");
pub const COMMUN_JS: &str = include_str!("../../../assets/js/commun.js");
pub const PIN_JS: &str = include_str!("../../../assets/js/pin.js");
pub const ZXING_JS: &str = include_str!("../../../assets/vendor/zxing-reader.js");
pub const ZXING_WASM: &[u8] = include_bytes!("../../../assets/vendor/zxing_reader.wasm");

/// Empreinte courte du contenu, utilisée pour purger les caches sans
/// jamais servir un fichier périmé.
pub fn version() -> &'static str {
    // Recalculée à chaque compilation : un déploiement invalide le cache.
    env!("EMPREINTE_STATIQUE")
}

fn reponse(type_mime: &'static str, corps: impl Into<Body>, immuable: bool) -> Response {
    let cache = if immuable {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=300"
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(type_mime)),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache)),
        ],
        corps.into(),
    )
        .into_response()
}

pub async fn css() -> Response {
    reponse("text/css; charset=utf-8", CSS, true)
}
pub async fn htmx() -> Response {
    reponse("text/javascript; charset=utf-8", HTMX, true)
}
pub async fn scan_js() -> Response {
    reponse("text/javascript; charset=utf-8", SCAN_JS, true)
}
pub async fn zxing_js() -> Response {
    reponse("text/javascript; charset=utf-8", ZXING_JS, true)
}
pub async fn zxing_wasm() -> Response {
    reponse("application/wasm", ZXING_WASM, true)
}
pub async fn commun_js() -> Response {
    reponse("text/javascript; charset=utf-8", COMMUN_JS, true)
}
pub async fn pin_js() -> Response {
    reponse("text/javascript; charset=utf-8", PIN_JS, true)
}
