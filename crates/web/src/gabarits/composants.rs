use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use domaine::Statut;
use maud::{html, Markup};

use crate::i18n::traduire;
use crate::t;

// ------------------------------------------------------------------ temps
pub fn fuseau(nom: &str) -> Tz {
    nom.parse().unwrap_or(chrono_tz::Europe::Paris)
}

pub fn date_heure(d: DateTime<Utc>, tz: &str) -> String {
    d.with_timezone(&fuseau(tz))
        .format(t!("temps.date_heure"))
        .to_string()
}

pub fn date(d: DateTime<Utc>, tz: &str) -> String {
    d.with_timezone(&fuseau(tz))
        .format(t!("temps.date"))
        .to_string()
}

pub fn jour(d: NaiveDate) -> String {
    d.format(t!("temps.date")).to_string()
}

/// « il y a 3 j » — plus lisible qu'une date absolue pour de l'activité.
pub fn depuis(d: DateTime<Utc>) -> String {
    let ecart = Utc::now().signed_duration_since(d);
    let minutes = ecart.num_minutes();
    if minutes < 1 {
        t!("temps.a_l_instant").to_string()
    } else if minutes < 60 {
        t!("temps.minutes", n = minutes)
    } else if ecart.num_hours() < 24 {
        t!("temps.heures", n = ecart.num_hours())
    } else {
        t!("temps.jours", n = ecart.num_days())
    }
}

pub fn option_date_heure(d: Option<DateTime<Utc>>, tz: &str) -> Markup {
    match d {
        Some(d) => html! { span class="num" { (date_heure(d, tz)) } },
        None => html! { span class="discret" { "—" } },
    }
}

pub fn texte_ou_tiret(v: Option<&String>) -> Markup {
    match v.filter(|s| !s.is_empty()) {
        Some(s) => html! { (s) },
        None => html! { span class="discret" { "—" } },
    }
}

// ------------------------------------------------------------------ badges
pub fn badge_statut(s: Statut) -> Markup {
    html! {
        span class={ "badge badge--" (s.code()) } { (traduire(s.cle_i18n())) }
    }
}

pub fn badge(ton: &str, texte: &str) -> Markup {
    html! { span class={ "badge badge--" (ton) } { (texte) } }
}

pub fn pastille_biere(couleur: Option<&String>) -> Markup {
    html! {
        @if let Some(c) = couleur {
            span class="pastille" style={ "background:" (c) } {}
        }
    }
}

// ------------------------------------------------------------------ formulaires
pub fn csrf(jeton: &str) -> Markup {
    html! { input type="hidden" name="csrf" value=(jeton); }
}

pub struct Champ<'a> {
    pub nom: &'a str,
    pub libelle: &'a str,
    pub valeur: String,
    pub type_html: &'a str,
    pub requis: bool,
    pub aide: Option<&'a str>,
    pub erreur: Option<&'a str>,
    pub attributs: Option<Markup>,
}

impl<'a> Champ<'a> {
    pub fn texte(nom: &'a str, libelle: &'a str, valeur: impl Into<String>) -> Self {
        Champ {
            nom,
            libelle,
            valeur: valeur.into(),
            type_html: "text",
            requis: false,
            aide: None,
            erreur: None,
            attributs: None,
        }
    }
    pub fn de_type(mut self, t: &'a str) -> Self {
        self.type_html = t;
        self
    }
    pub fn requis(mut self) -> Self {
        self.requis = true;
        self
    }
    pub fn aide(mut self, a: &'a str) -> Self {
        self.aide = Some(a);
        self
    }
    pub fn erreur(mut self, erreurs: &'a [(String, String)]) -> Self {
        self.erreur = erreurs
            .iter()
            .find(|(c, _)| c == self.nom)
            .map(|(_, m)| m.as_str());
        self
    }
    pub fn rendre(self) -> Markup {
        let id = format!("champ-{}", self.nom);
        html! {
            div class="champ" {
                label for=(id) {
                    (self.libelle)
                    @if self.requis { span class="discret" { " *" } }
                }
                input id=(id) type=(self.type_html) name=(self.nom) value=(self.valeur)
                      required[self.requis]
                      aria-invalid=[self.erreur.map(|_| "true")]
                      autocomplete="off";
                @if let Some(a) = self.aide { span class="champ__aide" { (a) } }
                @if let Some(e) = self.erreur { span class="champ__erreur" { (e) } }
            }
        }
    }
}

/// Options de liste déroulante issues d'un référentiel : (identifiant, libellé).
pub fn options<T>(
    elements: &[T],
    ident: impl Fn(&T) -> uuid::Uuid,
    libelle: impl Fn(&T) -> String,
) -> Vec<(String, String)> {
    elements
        .iter()
        .map(|x| (ident(x).to_string(), libelle(x)))
        .collect()
}

/// Liste déroulante. Les référentiels ne se saisissent jamais librement.
pub fn selection(
    nom: &str,
    libelle: &str,
    options: &[(String, String)],
    valeur: Option<&str>,
    vide: Option<&str>,
    requis: bool,
) -> Markup {
    let id = format!("champ-{nom}");
    html! {
        div class="champ" {
            label for=(id) { (libelle) @if requis { span class="discret" { " *" } } }
            select id=(id) name=(nom) required[requis] {
                @if let Some(v) = vide { option value="" { (v) } }
                @for (val, texte) in options {
                    option value=(val) selected[valeur == Some(val.as_str())] { (texte) }
                }
            }
        }
    }
}

pub fn zone_texte(nom: &str, libelle: &str, valeur: &str, aide: Option<&str>) -> Markup {
    let id = format!("champ-{nom}");
    html! {
        div class="champ" {
            label for=(id) { (libelle) }
            textarea id=(id) name=(nom) rows="3" { (valeur) }
            @if let Some(a) = aide { span class="champ__aide" { (a) } }
        }
    }
}

pub fn case_a_cocher(nom: &str, libelle: &str, coche: bool) -> Markup {
    let id = format!("champ-{nom}");
    html! {
        div class="champ" {
            label class="coche" for=(id) {
                input id=(id) type="checkbox" name=(nom) value="1" checked[coche];
                span { (libelle) }
            }
        }
    }
}

// ------------------------------------------------------------------ divers
pub fn vide(message: &str, action: Option<Markup>) -> Markup {
    html! {
        div class="vide" {
            p { (message) }
            @if let Some(a) = action { (a) }
        }
    }
}

pub fn pagination(page: i64, total_pages: i64, base: &str) -> Markup {
    if total_pages <= 1 {
        return html! {};
    }
    let lien = |p: i64| format!("{base}page={p}");
    html! {
        div class="pagination" {
            span class="petit discret" {
                (t!("parc.page", page = page, total = total_pages))
            }
            div class="boutons" {
                @if page > 1 {
                    a class="bouton bouton--petit" href=(lien(page - 1)) { (t!("action.precedent")) }
                }
                @if page < total_pages {
                    a class="bouton bouton--petit" href=(lien(page + 1)) { (t!("action.suivant")) }
                }
            }
        }
    }
}

/// En-tête de colonne cliquable pour trier.
pub fn entete_tri(libelle: &str, cle: &str, tri_actif: &str, desc: bool, base: &str) -> Markup {
    let actif = cle == tri_actif;
    let prochain_desc = if actif { !desc } else { false };
    let href = format!(
        "{base}tri={cle}&sens={}",
        if prochain_desc { "desc" } else { "asc" }
    );
    html! {
        th {
            a href=(href) {
                (libelle)
                @if actif { span aria-hidden="true" { @if desc { "↓" } @else { "↑" } } }
            }
        }
    }
}

/// Confirmation pour un formulaire classique (sans htmx) : attribut
/// `data-confirmer`, géré par `assets/js/commun.js` (la CSP interdit le
/// JavaScript en ligne). Maud échappe l'attribut HTML.
pub fn confirmer_soumission(message: &str) -> String {
    message.to_string()
}

/// Lien de retour en tête des sous-pages d'administration.
pub fn retour_administration() -> Markup {
    html! {
        p class="petit" style="margin:0 0 12px" {
            a href="/administration" { "← " (t!("admin.titre")) }
        }
    }
}
