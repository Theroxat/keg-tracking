use axum::extract::{FromRequestParts, Request, State};
use axum::http::{header, request::Parts, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use persistance::session::{self, ContexteSession};

use crate::{ErreurWeb, Etat};

/// Adresse réelle du client. Derrière le tunnel Cloudflare puis Caddy,
/// l'adresse vue par la socket est celle du proxy : la vraie se lit dans
/// les en-têtes transmis, et **seulement** si l'on a déclaré être derrière
/// un proxy de confiance.
pub fn adresse_client(req: &Request, derriere_proxy: bool) -> Option<String> {
    if derriere_proxy {
        for nom in ["cf-connecting-ip", "x-forwarded-for"] {
            if let Some(v) = req.headers().get(nom).and_then(|v| v.to_str().ok()) {
                if let Some(premier) = v.split(',').next() {
                    let ip = premier.trim();
                    if !ip.is_empty() {
                        return Some(ip.to_string());
                    }
                }
            }
        }
    }
    req.extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0.ip().to_string())
}

pub fn adresse_depuis_parts(parts: &Parts, derriere_proxy: bool) -> Option<String> {
    if derriere_proxy {
        for nom in ["cf-connecting-ip", "x-forwarded-for"] {
            if let Some(v) = parts.headers.get(nom).and_then(|v| v.to_str().ok()) {
                if let Some(premier) = v.split(',').next() {
                    let ip = premier.trim();
                    if !ip.is_empty() {
                        return Some(ip.to_string());
                    }
                }
            }
        }
    }
    None
}

fn cookie(parts_headers: &axum::http::HeaderMap, nom: &str) -> Option<String> {
    parts_headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|p| {
            let (k, val) = p.split_once('=')?;
            (k.trim() == nom).then(|| val.trim().to_string())
        })
        .next()
}

/// Résout le cookie de session et dépose le contexte dans la requête.
/// Ne refuse rien : la décision appartient aux extracteurs et aux gardes
/// de rôle, pour que les pages publiques restent accessibles.
pub async fn session_courante(State(etat): State<Etat>, mut req: Request, next: Next) -> Response {
    if let Some(jeton) = cookie(req.headers(), session::NOM_COOKIE) {
        match session::resoudre(etat.depot.auth(), &jeton).await {
            Ok(Some(mut ctx)) => {
                // Verrouillage par inactivité : la session reste valide, mais
                // elle redemande le code PIN.
                if !ctx.verrouillee && ctx.inactive_depuis_trop_longtemps() && ctx.a_un_pin {
                    let _ = session::verrouiller(etat.depot.auth(), ctx.session_id).await;
                    ctx.verrouillee = true;
                }
                req.extensions_mut().insert(ctx);
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(erreur = %e, "résolution de session"),
        }
    }
    next.run(req).await
}

/// En-têtes de sécurité, y compris une politique de sécurité de contenu
/// stricte : aucun script externe, aucun CDN.
pub async fn en_tetes_securite(req: Request, next: Next) -> Response {
    let mut reponse = next.run(req).await;
    let h = reponse.headers_mut();
    let csp = "default-src 'self'; \
               script-src 'self' 'wasm-unsafe-eval' 'unsafe-inline'; \
               style-src 'self' 'unsafe-inline'; \
               img-src 'self' data: blob:; \
               media-src 'self' blob:; \
               connect-src 'self'; \
               font-src 'self'; \
               object-src 'none'; \
               base-uri 'none'; \
               form-action 'self'; \
               frame-ancestors 'none'";
    h.insert("content-security-policy", HeaderValue::from_static(csp));
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("referrer-policy", HeaderValue::from_static("same-origin"));
    h.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(self), microphone=(), geolocation=()"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    reponse
}

/// Protection CSRF sur **toute** méthode non idempotente.
///
/// Le jeton est propre à la session et stocké en base ; il arrive soit par
/// le champ caché des formulaires, soit par l'en-tête `X-CSRF` que htmx
/// pose sur chaque requête. L'origine est vérifiée en complément.
pub async fn csrf(State(etat): State<Etat>, req: Request, next: Next) -> Response {
    if matches!(
        req.method(),
        &Method::GET | &Method::HEAD | &Method::OPTIONS
    ) {
        return next.run(req).await;
    }

    // Le contrôle d'origine s'applique à toutes les écritures, y compris
    // sur les routes publiques comme la connexion.
    if let Some(refus) = verifier_origine(&etat, &req) {
        return refus;
    }

    let attendu = req
        .extensions()
        .get::<ContexteSession>()
        .map(|c| c.jeton_csrf.clone())
        .or_else(|| {
            req.extensions()
                .get::<persistance::super_admin::ContexteEditeur>()
                .map(|c| c.jeton_csrf.clone())
        });
    let Some(attendu) = attendu else {
        return next.run(req).await;
    };

    let en_tete = req
        .headers()
        .get("x-csrf")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    if let Some(j) = en_tete {
        if constant_eq(&j, &attendu) {
            return next.run(req).await;
        }
        return ErreurWeb::Csrf.into_response();
    }

    // Formulaire classique : on lit le corps, on vérifie, on le remet.
    let (parts, corps) = req.into_parts();
    let octets = match axum::body::to_bytes(corps, 2 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "corps trop volumineux").into_response(),
    };
    let fourni = form_urlencoded_valeur(&octets, "csrf");
    if !fourni.map(|f| constant_eq(&f, &attendu)).unwrap_or(false) {
        return ErreurWeb::Csrf.into_response();
    }
    let req = Request::from_parts(parts, axum::body::Body::from(octets));
    next.run(req).await
}

/// Renvoie la réponse de refus si la requête vient d'un autre site.
fn verifier_origine(etat: &Etat, req: &Request) -> Option<Response> {
    // Sec-Fetch-Site est posé par tous les navigateurs modernes ; Origin
    // sert de repli. Une requête inter-site sur une méthode d'écriture est
    // refusée avant même d'examiner le jeton.
    if let Some(site) = req
        .headers()
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
    {
        if !matches!(site, "same-origin" | "same-site" | "none") {
            return Some(ErreurWeb::Csrf.into_response());
        }
        return None;
    }
    if let Some(origine) = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
    {
        let attendu = etat.config.url_publique();
        if origine != attendu && !origine.ends_with(&etat.config.domaine_public) {
            return Some(ErreurWeb::Csrf.into_response());
        }
    }
    None
}

fn constant_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn form_urlencoded_valeur(corps: &[u8], cle: &str) -> Option<String> {
    let texte = std::str::from_utf8(corps).ok()?;
    for paire in texte.split('&') {
        let (k, v) = paire.split_once('=')?;
        if k == cle {
            return urlencoding::decode(&v.replace('+', " "))
                .ok()
                .map(|s| s.into_owned());
        }
    }
    None
}

// ------------------------------------------------------------- extracteurs
/// Session valide et déverrouillée. Toute page applicative l'exige.
pub struct Session(pub ContexteSession);

impl<S: Send + Sync> FromRequestParts<S> for Session {
    type Rejection = ErreurWeb;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let ctx = parts
            .extensions
            .get::<ContexteSession>()
            .cloned()
            .ok_or(ErreurWeb::NonAuthentifie)?;
        if ctx.locataire_etat == persistance::locataire::EtatLocataire::Suspendu {
            return Err(ErreurWeb::Interdit);
        }
        if ctx.verrouillee {
            return Err(ErreurWeb::Verrouillee);
        }
        Ok(Session(ctx))
    }
}

/// Session éventuellement verrouillée : seul l'écran de déverrouillage
/// l'utilise.
pub struct SessionBrute(pub ContexteSession);

impl<S: Send + Sync> FromRequestParts<S> for SessionBrute {
    type Rejection = ErreurWeb;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ContexteSession>()
            .cloned()
            .map(SessionBrute)
            .ok_or(ErreurWeb::NonAuthentifie)
    }
}

/// Garde de rôle appliquée à un groupe de routes.
///
/// Une route ne peut pas être enregistrée en dehors d'un de ces groupes :
/// c'est ce qui garantit qu'aucune n'échappe au contrôle. Masquer un bouton
/// dans le gabarit reste du confort, jamais une protection.
pub async fn garde_role(
    State(role_requis): State<domaine::Role>,
    req: Request,
    next: Next,
) -> Response {
    let ctx = req.extensions().get::<ContexteSession>().cloned();
    match ctx {
        None => Redirect::to("/connexion").into_response(),
        Some(c) if c.locataire_etat == persistance::locataire::EtatLocataire::Suspendu => {
            ErreurWeb::Interdit.into_response()
        }
        Some(c) if c.verrouillee => Redirect::to("/verrouillage").into_response(),
        Some(c) if !c.role.permet(role_requis) => ErreurWeb::Interdit.into_response(),
        Some(_) => next.run(req).await,
    }
}

/// Lit le cookie d'avis déposé après une action, puis demande son effacement.
pub fn avis_depuis_cookie(parts: &Parts) -> Option<crate::gabarits::socle::Avis> {
    let brut = cookie(&parts.headers, "wq_avis")?;
    let decode = urlencoding::decode(&brut).ok()?.into_owned();
    let (ton, texte) = decode.split_once('|')?;
    let ton = match ton {
        "ok" => "ok",
        "alerte" => "alerte",
        _ => "grave",
    };
    Some(crate::gabarits::socle::Avis {
        ton,
        texte: texte.to_string(),
    })
}

pub fn cookie_avis(ton: &str, texte: &str, securise: bool) -> String {
    format!(
        "wq_avis={}; Path=/; Max-Age=20; SameSite=Lax; HttpOnly{}",
        urlencoding::encode(&format!("{ton}|{texte}")),
        if securise { "; Secure" } else { "" }
    )
}

pub fn cookie_avis_efface(securise: bool) -> String {
    format!(
        "wq_avis=; Path=/; Max-Age=0; SameSite=Lax; HttpOnly{}",
        if securise { "; Secure" } else { "" }
    )
}
