use maud::{html, Markup, DOCTYPE};
use persistance::session::ContexteSession;

use crate::i18n::traduire;
use crate::statique;
use crate::t;

/// Un message affiché en haut de page après une action.
pub struct Avis {
    pub ton: &'static str,
    pub texte: String,
}

impl Avis {
    pub fn ok(texte: impl Into<String>) -> Avis {
        Avis {
            ton: "ok",
            texte: texte.into(),
        }
    }
    pub fn alerte(texte: impl Into<String>) -> Avis {
        Avis {
            ton: "alerte",
            texte: texte.into(),
        }
    }
    pub fn grave(texte: impl Into<String>) -> Avis {
        Avis {
            ton: "grave",
            texte: texte.into(),
        }
    }
}

/// Tout ce que l'ossature doit savoir pour rendre une page.
pub struct Cadre<'a> {
    pub session: &'a ContexteSession,
    pub chemin: &'a str,
    pub titre: String,
    pub sous_titre: Option<String>,
    pub actions: Markup,
    pub avis: Vec<Avis>,
    pub assistance: Option<String>,
}

impl<'a> Cadre<'a> {
    pub fn new(session: &'a ContexteSession, chemin: &'a str, titre: impl Into<String>) -> Self {
        Cadre {
            session,
            chemin,
            titre: titre.into(),
            sous_titre: None,
            actions: html! {},
            avis: vec![],
            assistance: session.usurpee_par.map(|_| session.locataire_nom.clone()),
        }
    }
    pub fn sous_titre(mut self, s: impl Into<String>) -> Self {
        self.sous_titre = Some(s.into());
        self
    }
    pub fn actions(mut self, m: Markup) -> Self {
        self.actions = m;
        self
    }
    pub fn avis(mut self, a: Option<Avis>) -> Self {
        if let Some(a) = a {
            self.avis.push(a);
        }
        self
    }
}

fn tete(titre: &str) -> Markup {
    let v = statique::version();
    html! {
        meta charset="utf-8";
        meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
        meta name="color-scheme" content="light dark";
        meta name="theme-color" content="#f7f6f3" media="(prefers-color-scheme: light)";
        meta name="theme-color" content="#0f1214" media="(prefers-color-scheme: dark)";
        meta name="robots" content="noindex, nofollow";
        title { (titre) " — " (t!("app.nom")) }
        link rel="stylesheet" href={ "/statique/app.css?v=" (v) };
        link rel="icon" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E%3Ctext y='14' font-size='14'%3E%F0%9F%9B%A2%3C/text%3E%3C/svg%3E";
    }
}

/// Page sans ossature : connexion, verrouillage, erreurs, impression.
pub fn page_nue(titre: &str, corps: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="fr" {
            head { (tete(titre)) }
            body { (corps) }
        }
    }
}

const ICONES: &[(&str, &str)] = &[
    ("tableau", "M3 3h7v7H3zM14 3h7v4h-7zM14 10h7v11h-7zM3 13h7v8H3z"),
    ("parc", "M6 2h12l-1 5v13a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2V7zM5 7h14M5 12h14"),
    ("scan", "M3 7V5a2 2 0 0 1 2-2h2M17 3h2a2 2 0 0 1 2 2v2M21 17v2a2 2 0 0 1-2 2h-2M7 21H5a2 2 0 0 1-2-2v-2M7 12h10"),
    ("documents", "M8 2h8l4 4v14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2zM15 2v5h5M9 13h6M9 17h6"),
    ("clients", "M17 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2M9.5 7a3.5 3.5 0 1 1-7 0 3.5 3.5 0 0 1 7 0zM22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"),
    ("admin", "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6h.09A1.65 1.65 0 0 0 10.6 3.09V3a2 2 0 1 1 4 0v.09A1.65 1.65 0 0 0 16.11 4.6a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.09a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"),
    ("lots", "M12 2 3 7v10l9 5 9-5V7zM3 7l9 5 9-5M12 12v10"),
    ("plusieurs", "M8 8h12v12H8zM4 16V4h12"),
];

fn icone(nom: &str) -> Markup {
    let d = ICONES
        .iter()
        .find(|(n, _)| *n == nom)
        .map(|(_, d)| *d)
        .unwrap_or("");
    html! {
        svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"
            stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
            path d=(d) {}
        }
    }
}

struct Lien {
    href: &'static str,
    cle: &'static str,
    icone: &'static str,
    role_min: domaine::Role,
}

const LIENS: &[Lien] = &[
    Lien {
        href: "/",
        cle: "nav.tableau_de_bord",
        icone: "tableau",
        role_min: domaine::Role::Lecture,
    },
    Lien {
        href: "/scan",
        cle: "nav.scan",
        icone: "scan",
        role_min: domaine::Role::Production,
    },
    Lien {
        href: "/parc",
        cle: "nav.parc",
        icone: "parc",
        role_min: domaine::Role::Lecture,
    },
    Lien {
        href: "/documents",
        cle: "nav.documents",
        icone: "documents",
        role_min: domaine::Role::Lecture,
    },
    Lien {
        href: "/clients",
        cle: "nav.clients",
        icone: "clients",
        role_min: domaine::Role::Lecture,
    },
    Lien {
        href: "/lots",
        cle: "nav.lots",
        icone: "lots",
        role_min: domaine::Role::Lecture,
    },
    Lien {
        href: "/administration",
        cle: "nav.administration",
        icone: "admin",
        role_min: domaine::Role::Administrateur,
    },
];

/// Entrée de la barre du bas. Elle ne reprend pas la navigation du flanc :
/// l'atelier n'a besoin que de trois gestes, le bureau garde ses listes.
struct LienMobile {
    href: &'static str,
    /// Préfixe qui rend l'entrée active (« /operations » couvre tout le
    /// parcours, alors que le lien mène à « /operations/nouvelle »).
    actif: &'static str,
    /// Libellé court : la place y est comptée.
    cle: &'static str,
    icone: &'static str,
    /// Le scan est le geste principal : plus grand, coloré, au centre.
    central: bool,
}

const M_ACCUEIL: LienMobile = LienMobile {
    href: "/",
    actif: "/",
    cle: "nav.court.tableau_de_bord",
    icone: "tableau",
    central: false,
};
const M_SCAN: LienMobile = LienMobile {
    href: "/scan",
    actif: "/scan",
    cle: "nav.court.scan",
    icone: "scan",
    central: true,
};
const M_LOT: LienMobile = LienMobile {
    href: "/operations/nouvelle",
    actif: "/operations",
    cle: "nav.court.operation",
    icone: "plusieurs",
    central: false,
};
const M_PARC: LienMobile = LienMobile {
    href: "/parc",
    actif: "/parc",
    cle: "nav.court.parc",
    icone: "parc",
    central: false,
};
const M_BONS: LienMobile = LienMobile {
    href: "/documents",
    actif: "/documents",
    cle: "nav.court.documents",
    icone: "documents",
    central: false,
};
const M_CLIENTS: LienMobile = LienMobile {
    href: "/clients",
    actif: "/clients",
    cle: "nav.court.clients",
    icone: "clients",
    central: false,
};

/// La barre du bas dépend du rôle, pas de la largeur : un opérateur de
/// production n'y voit que l'accueil, le scan et l'opération par lot.
fn liens_mobiles(role: domaine::Role) -> &'static [LienMobile] {
    match role {
        domaine::Role::Production => &[M_ACCUEIL, M_SCAN, M_LOT],
        domaine::Role::Administrateur => &[M_ACCUEIL, M_PARC, M_SCAN, M_BONS, M_CLIENTS],
        domaine::Role::Lecture => &[M_ACCUEIL, M_PARC, M_BONS, M_CLIENTS],
    }
}

fn actif(chemin: &str, href: &str) -> bool {
    if href == "/" {
        chemin == "/"
    } else {
        chemin == href || chemin.starts_with(&format!("{href}/"))
    }
}

/// Ossature complète de l'application.
///
/// Le bureau et l'atelier ne sont pas deux largeurs d'écran : ce sont deux
/// jeux de liens et de gabarits, choisis d'après le rôle. Aucune détection
/// de largeur en JavaScript — le serveur sait qui demande.
pub fn page(cadre: Cadre<'_>, corps: Markup) -> Markup {
    let role = cadre.session.role;
    let v = statique::version();
    html! {
        (DOCTYPE)
        html lang="fr" {
            head { (tete(&cadre.titre)) }
            body hx-headers=(format!(r#"{{"X-CSRF":"{}"}}"#, cadre.session.jeton_csrf)) {
                @if let Some(brasserie) = &cadre.assistance {
                    div class="bandeau-assistance" {
                        (t!("editeur.usurpation_active", brasserie = brasserie))
                        " "
                        form method="post" action="/editeur/assistance/arreter" style="display:inline" {
                            input type="hidden" name="csrf" value=(cadre.session.jeton_csrf);
                            button type="submit" class="bouton bouton--petit bouton--nu" style="color:#fff" {
                                (t!("editeur.usurpation_arreter"))
                            }
                        }
                    }
                }
                div class="appli" {
                    nav class="flanc" aria-label=(t!("nav.tableau_de_bord")) {
                        div class="flanc__marque" {
                            b { (cadre.session.locataire_nom) }
                            span { (t!("app.baseline")) }
                        }
                        form class="flanc__recherche" method="get" action="/recherche" role="search" {
                            input type="search" name="q" id="recherche-globale"
                                placeholder=(t!("recherche.placeholder"))
                                aria-label=(t!("action.rechercher")) autocomplete="off";
                        }
                        div class="flanc__nav" {
                            @for l in LIENS {
                                @if role.permet(l.role_min) {
                                    a href=(l.href)
                                      aria-current=[actif(cadre.chemin, l.href).then_some("page")] {
                                        (icone(l.icone)) span { (traduire(l.cle)) }
                                    }
                                }
                            }
                        }
                        div class="flanc__pied" {
                            div class="flanc__moi" {
                                b { (cadre.session.nom_complet()) }
                                span { (traduire(role.cle_i18n())) }
                            }
                            a href="/compte" { (t!("nav.mon_compte")) }
                            form method="post" action="/deconnexion" {
                                input type="hidden" name="csrf" value=(cadre.session.jeton_csrf);
                                button type="submit" class="bouton bouton--nu bouton--petit bouton--large" {
                                    (t!("nav.deconnexion"))
                                }
                            }
                        }
                    }
                    div class="corps" {
                        header class="entete" {
                            div class="entete__titre" {
                                h1 { (cadre.titre) }
                                @if let Some(s) = &cadre.sous_titre { p { (s) } }
                            }
                            div class="boutons sans-impression" { (cadre.actions) }
                        }
                        main class="contenu" {
                            @for a in &cadre.avis {
                                div class={ "avis avis--" (a.ton) } role="status" { (a.texte) }
                            }
                            (corps)
                        }
                    }
                }
                nav class="barre-mobile" aria-label=(t!("nav.tableau_de_bord")) {
                    @for l in liens_mobiles(role) {
                        a href=(l.href)
                          class=[l.central.then_some("barre-mobile__central")]
                          aria-current=[actif(cadre.chemin, l.actif).then_some("page")] {
                            (icone(l.icone)) span { (traduire(l.cle)) }
                        }
                    }
                }
                script src={ "/statique/htmx.js?v=" (v) } defer {}
                script src={ "/statique/commun.js?v=" (v) } defer {}
            }
        }
    }
}
