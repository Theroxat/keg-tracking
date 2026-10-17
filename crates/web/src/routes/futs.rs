use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Form;
use domaine::{ContexteTransition, Origine, Role, Statut};
use maud::{html, Markup};
use serde::Deserialize;
use uuid::Uuid;

use crate::gabarits::composants as c;
use crate::gabarits::csrf;
use crate::gabarits::socle::{page, Cadre};
use crate::i18n::traduire;
use crate::middleware::Session;
use crate::routes::{page_avec_avis, rediriger, uuid_opt, Contexte};
use crate::{t, ErreurWeb, Etat};

pub async fn fiche(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis.is_some();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let f = persistance::fut::lire(tx.conn(), id).await?;
    let chronologie = persistance::evenement::chronologie(tx.conn(), id).await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;

    let bieres = persistance::referentiel::lister_bieres(tx.conn(), true).await?;
    let cuves = persistance::referentiel::lister_cuves(tx.conn(), true).await?;
    let emplacements = persistance::referentiel::lister_emplacements(tx.conn(), true).await?;
    let clients = persistance::client::lister(tx.conn(), false, None).await?;
    let lots = persistance::lot::lister(tx.conn(), None, None, None).await?;
    let autres_futs: Vec<_> = match f.client_id {
        Some(cid) => persistance::fut::detenus_par_client(tx.conn(), cid).await?,
        None => vec![],
    }
    .into_iter()
    .filter(|v| v.id != id)
    .collect();
    let adresses = match f.client_id {
        Some(cid) => persistance::client::lister_adresses(tx.conn(), cid, false).await?,
        None => vec![],
    };
    tx.valider().await?;

    let peut_ecrire = s.role.peut_ecrire();
    let est_admin = s.role == Role::Administrateur;
    let tz = s.fuseau_horaire.clone();

    let opts_bieres: Vec<(String, String)> = c::options(&bieres, |x| x.id, |x| x.nom.clone());
    let opts_cuves: Vec<(String, String)> = c::options(&cuves, |x| x.id, |x| x.nom.clone());
    let opts_empl: Vec<(String, String)> = c::options(&emplacements, |x| x.id, |x| x.nom.clone());
    let opts_clients: Vec<(String, String)> = c::options(&clients, |x| x.id, |x| x.societe.clone());
    let opts_lots: Vec<(String, String)> = lots
        .iter()
        .map(|l| (l.id.to_string(), format!("{} — {}", l.numero, l.biere)))
        .collect();
    let opts_adresses: Vec<(String, String)> = adresses
        .iter()
        .map(|a| {
            (
                a.id.to_string(),
                format!("{} — {}", a.libelle, a.une_ligne()),
            )
        })
        .collect();

    let corps = html! {
        // Trois blocs placés par zones : sur téléphone la grille bascule en
        // une colonne et remonte les actions avant la chronologie, qui peut
        // faire soixante entrées.
        div class="fiche" {
            div class="fiche__etat" {
                // --------------------------------------------- état courant
                (carte_etat(&f))
            }

            div class="fiche__details" {
                (carte_details(&f, &loc, &tz))
            }

            // --------------------------------------------- autres fûts du client
            @if !autres_futs.is_empty() {
                div class="fiche__autres" {
                    (carte_autres_futs(&autres_futs, f.client_id))
                }
            }

            div class="fiche__chrono" {
                // --------------------------------------------- chronologie
                (carte_chronologie(&chronologie, &tz))
            }

            // --------------------------------------------- actions
            div class="fiche__actions sans-impression" {
                @if peut_ecrire {
                    section class="carte" {
                        div class="carte__tete" { h2 { (t!("fut.actions")) } }
                        div class="carte__corps" {
                            (formulaire_transition(&s, &f, &opts_bieres, &opts_lots, &opts_cuves,
                                                   &opts_empl, &opts_clients, &opts_adresses))
                        }
                    }
                    @if est_admin {
                        section class="carte carte--forcage" {
                            div class="carte__corps" {
                                details {
                                    summary class="bouton bouton--grave bouton--large fiche__resume" {
                                        (t!("action.forcer"))
                                    }
                                    p class="petit discret" style="margin-top:12px" { (t!("fut.forcage_avertissement")) }
                                    form method="post" action=(format!("/futs/{}/transition", f.id)) {
                                        (csrf(&s.jeton_csrf))
                                        input type="hidden" name="forcer" value="1";
                                        (c::selection("statut", t!("fut.transition_vers"),
                                            &Statut::TOUS.iter().filter(|x| **x != f.statut)
                                                .map(|x| (x.code().to_string(), traduire(x.cle_i18n()).to_string()))
                                                .collect::<Vec<_>>(), None, Some("—"), true))
                                        (c::zone_texte("justification", t!("champ.justification"), "", None))
                                        button type="submit" class="bouton bouton--grave bouton--large" {
                                            (t!("fut.forcage_confirmer"))
                                        }
                                    }
                                }
                            }
                        }
                    }
                    section class="carte" {
                        details class="fiche-details" {
                            summary class="fiche__resume" { (t!("fut.modifier_infos")) }
                            div class="carte__corps" {
                                form method="post" action=(format!("/futs/{}/identite", f.id)) {
                                    (csrf(&s.jeton_csrf))
                                    (c::selection("contenance_litres", t!("champ.contenance"),
                                        &loc.contenances_autorisees.iter()
                                            .map(|l| (l.to_string(), format!("{l} L"))).collect::<Vec<_>>(),
                                        Some(&f.contenance_litres.to_string()), None, true))
                                    (c::Champ::texte("date_mise_en_service", t!("champ.date_mise_en_service"),
                                        f.date_mise_en_service.to_string()).de_type("date").rendre())
                                    (c::zone_texte("commentaire", t!("champ.commentaire"), &f.commentaire, None))
                                    button type="submit" class="bouton bouton--large" { (t!("action.enregistrer")) }
                                }
                            }
                        }
                    }
                }
                @if let (true, Some(n)) = (est_admin, numero_etiquette(&f.code_qr, &loc.prefixe_qr)) {
                    a class="bouton bouton--large" href=(format!("/administration/etiquettes/planche?de={n}&a={n}")) {
                        (t!("fut.reimprimer_etiquette"))
                    }
                }
            }
        }
    };

    let cadre = Cadre::new(&s, &ctx.chemin, t!("fut.titre", code = &f.code_qr))
        .sous_titre(format!(
            "{} — {} {}",
            traduire(f.statut.cle_i18n()),
            f.contenance_litres,
            t!("unite.litres")
        ))
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

/// Bandeau de tête de la fiche : statut en grand, code, bière et client.
fn carte_etat(f: &persistance::fut::Fut) -> Markup {
    html! {
        section class="fiche-bandeau" {
            span class={ "fiche-bandeau__statut badge badge--" (f.statut.code()) } { (traduire(f.statut.cle_i18n())) }
            div class="fiche-bandeau__code mono" { (f.code_qr) }
            div class="fiche-bandeau__ligne" {
                @if let Some(b) = &f.biere {
                    span { (c::pastille_biere(f.couleur_biere.as_ref())) " " (b) }
                }
                @if let (Some(cl), Some(cid)) = (&f.client, f.client_id) {
                    a href=(format!("/clients/{cid}")) { (cl) }
                }
            }
            div class="petit discret" { (c::depuis(f.statut_depuis_le)) }
        }
    }
}

/// Détails de l'état courant, repliés dans un « Détails ».
fn carte_details(
    f: &persistance::fut::Fut,
    loc: &persistance::locataire::Locataire,
    tz: &str,
) -> Markup {
    html! {
    section class="carte" {
        details class="fiche-details" {
            summary class="fiche__resume" { (t!("fut.details")) }
            div class="carte__corps" {
            dl style="display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,145px),1fr));gap:12px;margin:0" {
                (paire(t!("champ.contenance"), html! { (f.contenance_litres) " " (t!("unite.litres")) }))
                (paire(t!("champ.depuis"), html! { (c::depuis(f.statut_depuis_le)) }))
                (paire(t!("champ.biere"), html! {
                    @if let Some(b) = &f.biere {
                        span style="display:inline-flex;align-items:center;gap:6px" {
                            (c::pastille_biere(f.couleur_biere.as_ref())) (b) }
                    } @else { span class="discret" { "—" } }
                }))
                (paire(t!("champ.lot"), html! {
                    @if let (Some(l), Some(lid)) = (&f.lot, f.lot_id) {
                        a href=(format!("/parc?lot={lid}")) class="mono" { (l) }
                    } @else { span class="discret" { "—" } }
                }))
                (paire(t!("champ.dluo"), html! {
                    @match f.dluo {
                        Some(d) => {
                            @let reste = (d - chrono::Utc::now().date_naive()).num_days();
                            @if reste < 0 { (c::badge("grave", t!("lot.dluo_depassee"))) }
                            @else if reste < loc.seuil_alerte_dluo_jours as i64 {
                                (c::badge("alerte", &c::jour(d)))
                            } @else { span class="num" { (c::jour(d)) } }
                        },
                        None => span class="discret" { "—" },
                    }
                }))
                (paire(t!("champ.cuve"), html! { (c::texte_ou_tiret(f.cuve.as_ref())) }))
                (paire(t!("champ.emplacement"), html! { (c::texte_ou_tiret(f.emplacement.as_ref())) }))
                (paire(t!("champ.client"), html! {
                    @if let (Some(cl), Some(cid)) = (&f.client, f.client_id) {
                        a href=(format!("/clients/{cid}")) { (cl) }
                    } @else { span class="discret" { "—" } }
                }))
                (paire(t!("champ.date_remplissage"), c::option_date_heure(f.date_remplissage, tz)))
                (paire(t!("champ.lave_le"), html! {
                    @match f.dernier_lavage_le {
                        Some(d) => {
                            span class="num" { (c::date_heure(d, tz)) }
                            @if let Some(q) = &f.dernier_lavage_par {
                                span class="discret petit" { " — " (q) }
                            }
                        },
                        None => span class="discret" { "—" },
                    }
                }))
                (paire(t!("champ.date_mise_en_service"), html! { span class="num" { (c::jour(f.date_mise_en_service)) } }))
            }
            @if !f.commentaire.is_empty() {
                p class="petit" style="margin-top:12px;padding-top:12px;border-top:1px solid var(--bordure)" {
                    (f.commentaire)
                }
            }
            }
        }
    }
    }
}

/// Autres fûts du même client : cinq au plus, le reste via le parc filtré.
fn carte_autres_futs(
    voisins: &[persistance::fut::FutImmobilise],
    client_id: Option<Uuid>,
) -> Markup {
    html! {
        section class="carte" {
            details class="fiche-details fiche-voisins" open {
                summary class="fiche__resume" { (t!("fut.autres_futs_client")) }
                div class="carte__corps carte__corps--serre" {
                    div class="tableau-cadre" style="border:0" {
                        table class="tableau tableau--cartes" { tbody {
                            @for v in voisins.iter().take(5) {
                                tr {
                                    td class="cel-titre" { a class="lien-ligne" href=(format!("/futs/{}", v.id)) { (v.code_qr) } }
                                    td class="cel-marque serre petit discret num" { (v.jours) " " (t!("tdb.jours")) }
                                }
                            }
                        } }
                    }
                    @if let (true, Some(cid)) = (voisins.len() > 5, client_id) {
                        p class="carte__corps" style="margin:0" {
                            a href=(format!("/parc?client={cid}")) {
                                (t!("fut.voir_futs_client", n = voisins.len()))
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Carte « chronologie » de la fiche fût.
fn carte_chronologie(
    chronologie: &[persistance::evenement::EntreeChronologie],
    tz: &str,
) -> Markup {
    html! {
    section class="carte" {
        div class="carte__tete" {
            h2 { (t!("fut.chronologie")) }
            span class="petit discret" { (t!("fut.nb_evenements", n = chronologie.len())) }
        }
        div class="carte__corps" {
            @if chronologie.is_empty() {
                p class="discret" { (t!("fut.aucun_evenement")) }
            } @else {
                ul class="chrono" {
                    @for e in chronologie.iter().take(10) { (evenement(e, tz)) }
                }
                @if chronologie.len() > 10 {
                    details class="fiche-historique" {
                        summary class="fiche__resume" { (t!("fut.historique_complet", n = chronologie.len())) }
                        ul class="chrono" {
                            @for e in chronologie.iter().skip(10) { (evenement(e, tz)) }
                        }
                    }
                }
            }
        }
    }
    }
}

fn evenement(e: &persistance::evenement::EntreeChronologie, tz: &str) -> Markup {
    html! {
                        li {
                            div class="chrono__point"
                                style=[e.statut_apres.map(|st| format!("background:var(--st-{})", st.code()))] {}
                            div {
                                div class="chrono__tete" {
                                    @if let Some(st) = e.statut_apres { (c::badge_statut(st)) }
                                    @else { span class="badge badge--neutre" { (traduire(e.type_evenement.cle_i18n())) } }
                                    @if e.type_evenement == domaine::TypeEvenement::TransitionForcee {
                                        (c::badge("grave", traduire("evenement.transition_forcee")))
                                    }
                                    span class="chrono__quand" { (c::date_heure(e.survenu_le, tz)) }
                                    span class="petit discret" { "· " (traduire(e.origine.cle_i18n())) }
                                    @if let Some(r) = &e.document_reference {
                                        span class="petit mono discret" { "· " (r) }
                                    }
                                }
                                div class={ "chrono__detail" @if e.type_evenement == domaine::TypeEvenement::TransitionForcee { " chrono__forcee" } @else { "" } } {
                                    @if let Some(x) = &e.client { (t!("champ.client")) ": " (x) ". " }
                                    @if let Some(x) = &e.lot { (t!("champ.lot")) ": " (x) ". " }
                                    @if let Some(x) = &e.biere { (x) ". " }
                                    @if let Some(x) = &e.emplacement { (t!("champ.emplacement")) ": " (x) ". " }
                                    @if !e.commentaire.is_empty() { (e.commentaire) " " }
                                    @if !e.justification.is_empty() {
                                        b { (t!("champ.justification")) " : " } (e.justification)
                                    }
                                    @if let Some(q) = &e.par { span class="discret" { " — " (q) } }
                                }
                            }
                        }
    }
}

fn paire(libelle: &str, valeur: Markup) -> Markup {
    html! {
        div { dt class="minus" { (libelle) } dd style="margin:2px 0 0;font-weight:520" { (valeur) } }
    }
}

/// Numéro de l'étiquette déduit du code QR (préfixe du locataire + numéro).
fn numero_etiquette(code: &str, prefixe: &str) -> Option<i64> {
    code.strip_prefix(prefixe)?.parse().ok()
}

/// Libellé verbal de l'action menant au statut cible.
fn libelle_action(st: Statut) -> &'static str {
    match st {
        Statut::Recu => t!("fut.action.recu"),
        Statut::Sale => t!("fut.action.sale"),
        Statut::Propre => t!("fut.action.propre"),
        Statut::Rempli => t!("fut.action.rempli"),
        Statut::Livre => t!("fut.action.livre"),
        Statut::Perdu => t!("fut.action.perdu"),
        Statut::HorsService => t!("fut.action.hors_service"),
    }
}

/// Étape normale suivante du cycle Reçu → Sale → Propre → Rempli → Livré → Reçu.
fn etape_suivante(st: Statut) -> Option<Statut> {
    match st {
        Statut::Recu => Some(Statut::Sale),
        Statut::Sale => Some(Statut::Propre),
        Statut::Propre => Some(Statut::Rempli),
        Statut::Rempli => Some(Statut::Livre),
        Statut::Livre => Some(Statut::Recu),
        Statut::HorsService | Statut::Perdu => None,
    }
}

/// Actions contextuelles : seules les transitions réellement possibles sont
/// proposées (`domaine::statut`). L'étape normale suivante est le gros bouton ;
/// les autres sont repliées, les actions graves viennent en dernier.
#[allow(clippy::too_many_arguments)]
fn formulaire_transition(
    s: &persistance::session::ContexteSession,
    f: &persistance::fut::Fut,
    bieres: &[(String, String)],
    lots: &[(String, String)],
    cuves: &[(String, String)],
    emplacements: &[(String, String)],
    clients: &[(String, String)],
    adresses: &[(String, String)],
) -> Markup {
    let valides = f.statut.transitions_valides();
    let principale = etape_suivante(f.statut).filter(|p| valides.contains(p));
    let graves = |st: &Statut| matches!(st, Statut::Perdu | Statut::HorsService);
    let action = |st: Statut, vif: bool| {
        action_transition(
            s,
            f,
            st,
            vif,
            bieres,
            lots,
            cuves,
            emplacements,
            clients,
            adresses,
        )
    };
    let autres: Vec<Statut> = valides
        .iter()
        .copied()
        .filter(|st| Some(*st) != principale && !graves(st))
        .collect();
    html! {
        @if let Some(p) = principale { (action(p, true)) }
        @if !autres.is_empty() {
            details class="fiche-autres" {
                summary class="bouton bouton--large fiche__resume" { (t!("fut.autre_action")) }
                @for st in &autres { (action(*st, false)) }
            }
        }
        @for st in valides.iter().filter(|st| graves(st)) { (action(*st, false)) }
    }
}

#[allow(clippy::too_many_arguments)]
fn action_transition(
    s: &persistance::session::ContexteSession,
    f: &persistance::fut::Fut,
    st: Statut,
    vif: bool,
    bieres: &[(String, String)],
    lots: &[(String, String)],
    cuves: &[(String, String)],
    emplacements: &[(String, String)],
    clients: &[(String, String)],
    adresses: &[(String, String)],
) -> Markup {
    let libelle = libelle_action(st);
    let classe_resume = if vif {
        "bouton bouton--vif bouton--atelier fiche__resume"
    } else if matches!(st, Statut::Perdu | Statut::HorsService) {
        "bouton bouton--grave bouton--large fiche__resume"
    } else {
        "bouton bouton--large fiche__resume"
    };
    let classe_bouton = if vif {
        "bouton bouton--vif bouton--atelier"
    } else {
        "bouton bouton--large"
    };
    html! {
        @match st {
            Statut::Rempli => {
                details class="fiche-action" {
                    summary class=(classe_resume) { (libelle) }
                    form method="post" action=(format!("/futs/{}/transition", f.id))
                         style="padding:10px 0 4px" {
                        (csrf(&s.jeton_csrf))
                        input type="hidden" name="statut" value=(st.code());
                        (c::selection("type_biere_id", t!("champ.type_biere"), bieres, None, Some("—"), true))
                        (c::selection("lot_id", t!("champ.lot"), lots, None, Some("—"), true))
                        (c::selection("cuve_id", t!("champ.cuve"), cuves, None, Some("—"), true))
                        button class="bouton bouton--vif bouton--large" { (t!("action.valider")) }
                    }
                }
            },
            Statut::Livre => {
                details class="fiche-action" {
                    summary class=(classe_resume) { (libelle) }
                    form method="post" action=(format!("/futs/{}/transition", f.id))
                         style="padding:10px 0 4px" {
                        (csrf(&s.jeton_csrf))
                        input type="hidden" name="statut" value=(st.code());
                        div class="champ" {
                            label for="champ-client_id" { (t!("champ.client")) span class="discret" { " *" } }
                            select id="champ-client_id" name="client_id" required
                                   hx-get="/clients/adresses"
                                   hx-target="#choix-adresse" hx-trigger="change"
                                   hx-include="this" {
                                option value="" { "—" }
                                @for (v, n) in clients { option value=(v) { (n) } }
                            }
                        }
                        div id="choix-adresse" {
                            (c::selection("adresse_client_id", t!("champ.adresse"), adresses, None, Some("—"), true))
                        }
                        button class="bouton bouton--vif bouton--large" { (t!("action.valider")) }
                    }
                }
            },
            Statut::Perdu | Statut::HorsService => {
                details class="fiche-action" {
                    summary class=(classe_resume) { (libelle) }
                    form method="post" action=(format!("/futs/{}/transition", f.id))
                         style="padding:10px 0 4px" {
                        (csrf(&s.jeton_csrf))
                        input type="hidden" name="statut" value=(st.code());
                        (c::zone_texte("justification", t!("champ.justification"), "", None))
                        button class="bouton bouton--grave bouton--large" { (t!("action.valider")) }
                    }
                }
            },
            _ => {
                form method="post" action=(format!("/futs/{}/transition", f.id))
                     class="fiche-action" {
                    (csrf(&s.jeton_csrf))
                    input type="hidden" name="statut" value=(st.code());
                    button class=(classe_bouton) { (libelle) }
                    @if st == Statut::Recu || st == Statut::Propre {
                        select name="emplacement_id" class="fiche__resume" style="margin-top:6px" {
                            option value="" { (t!("champ.emplacement")) " — " (t!("app.aucun")) }
                            @for (v, n) in emplacements { option value=(v) { (n) } }
                        }
                    }
                }
            },
        }
    }
}

#[derive(Deserialize)]
pub struct FormTransition {
    statut: String,
    #[serde(default)]
    type_biere_id: Option<String>,
    #[serde(default)]
    lot_id: Option<String>,
    #[serde(default)]
    cuve_id: Option<String>,
    #[serde(default)]
    emplacement_id: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    adresse_client_id: Option<String>,
    #[serde(default)]
    justification: Option<String>,
    #[serde(default)]
    commentaire: Option<String>,
    #[serde(default)]
    forcer: Option<String>,
}

pub async fn transition(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
    Form(f): Form<FormTransition>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let cible = Statut::depuis_code(&f.statut).ok_or(ErreurWeb::Introuvable)?;
    let securise = etat.config.cookies_securises;

    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let mut demande =
        persistance::evenement::DemandeEvenement::nouvelle(id, Origine::EditionManuelle);
    demande.statut_cible = Some(cible);
    demande.commentaire = f.commentaire.clone().unwrap_or_default();
    demande.ctx = ContexteTransition {
        type_biere_id: uuid_opt(&f.type_biere_id),
        lot_id: uuid_opt(&f.lot_id),
        cuve_id: uuid_opt(&f.cuve_id),
        client_id: uuid_opt(&f.client_id),
        adresse_client_id: uuid_opt(&f.adresse_client_id),
        emplacement_id: uuid_opt(&f.emplacement_id),
        justification: f.justification.clone(),
        forcee: f.forcer.as_deref() == Some("1"),
    };

    match persistance::evenement::enregistrer(&mut tx, s.role, &demande).await {
        Ok(_) => {
            tx.valider().await?;
            Ok(rediriger(
                &format!("/futs/{id}"),
                "ok",
                t!("fut.enregistre"),
                securise,
            ))
        }
        Err(e) => {
            // La transaction est abandonnée : rien n'est écrit.
            let message = match &e {
                persistance::Erreur::Metier(m) => crate::i18n::message_erreur(m),
                autre => {
                    tracing::warn!(erreur = %autre, "transition refusée");
                    t!("app.erreur_serveur").to_string()
                }
            };
            Ok(rediriger(
                &format!("/futs/{id}"),
                "grave",
                &message,
                securise,
            ))
        }
    }
}

#[derive(Deserialize)]
pub struct FormIdentite {
    contenance_litres: i32,
    date_mise_en_service: String,
    #[serde(default)]
    commentaire: String,
}

pub async fn identite(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
    Form(f): Form<FormIdentite>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let securise = etat.config.cookies_securises;
    let date = chrono::NaiveDate::parse_from_str(&f.date_mise_en_service, "%Y-%m-%d")
        .map_err(|_| ErreurWeb::champ("date_mise_en_service", t!("erreur.date_invalide")))?;

    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    domaine::contenance::verifier(f.contenance_litres, &loc.contenances_autorisees)?;
    persistance::fut::modifier_identite(
        tx.conn(),
        id,
        f.contenance_litres,
        date,
        &f.commentaire,
        Some(s.utilisateur_id),
    )
    .await?;
    tx.valider().await?;
    Ok(rediriger(
        &format!("/futs/{id}"),
        "ok",
        t!("fut.enregistre"),
        securise,
    ))
}

#[derive(Deserialize)]
pub struct RequeteAdresses {
    client_id: Option<Uuid>,
}

/// Fragment htmx : le choix d'un client recharge ses adresses, et l'adresse
/// unique est présélectionnée automatiquement.
pub async fn adresses_du_client(
    State(etat): State<Etat>,
    Session(s): Session,
    Query(q): Query<RequeteAdresses>,
) -> Result<Response, ErreurWeb> {
    let Some(client_id) = q.client_id else {
        return Ok(html! {
            (c::selection("adresse_client_id", t!("champ.adresse"), &[], None, Some("—"), true))
        }
        .into_response());
    };
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let adresses = persistance::client::lister_adresses(tx.conn(), client_id, false).await?;
    let unique = persistance::client::adresse_unique(tx.conn(), client_id).await?;
    tx.valider().await?;

    let options: Vec<(String, String)> = adresses
        .iter()
        .map(|a| {
            (
                a.id.to_string(),
                format!("{} — {}", a.libelle, a.une_ligne()),
            )
        })
        .collect();
    let choisie = unique.map(|u| u.to_string());
    Ok(html! {
        (c::selection("adresse_client_id", t!("champ.adresse"), &options,
                      choisie.as_deref(), if unique.is_some() { None } else { Some("—") }, true))
    }
    .into_response())
}
