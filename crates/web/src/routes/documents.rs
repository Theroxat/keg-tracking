//! Opérations par lot : le cœur du gain de temps.
//!
//! Le brouillon vit côté serveur dès la première capture. Rafraîchissement,
//! veille du téléphone, batterie à plat, changement d'appareil : on reprend
//! là où on s'était arrêté. C'est la raison pour laquelle le produit n'a
//! pas besoin d'une couche hors-ligne.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::{Form, Json};
use domaine::{EtatDocument, Statut, TypeDocument};
use maud::{html, Markup};
use serde::Deserialize;
use uuid::Uuid;

use crate::gabarits::composants as c;
use crate::gabarits::csrf;
use crate::gabarits::socle::{page, page_nue, Cadre};
use crate::i18n::traduire;
use crate::middleware::Session;
use crate::routes::scan::{ilot, Demande, Reponse};
use crate::routes::{page_avec_avis, rediriger, uuid_opt, Contexte};
use crate::{t, ErreurWeb, Etat};

use persistance::document::{self, Configuration, ResultatCapture};

// ------------------------------------------------------------------ liste
pub async fn liste(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let docs = document::lister(tx.conn(), None, 100).await?;
    tx.valider().await?;
    let tz = s.fuseau_horaire.clone();

    let corps = html! {
        @if docs.is_empty() {
            (c::vide(t!("op.aucun_document"), Some(html! {
                a class="bouton bouton--vif" href="/operations/nouvelle" { (t!("nav.operation")) }
            })))
        } @else {
            div class="tableau-cadre" {
                table class="tableau tableau--cartes" {
                    thead { tr {
                        th { (t!("champ.numero")) } th { "Type" } th { (t!("champ.statut")) }
                        th { "Fûts" } th { (t!("champ.client")) } th { "Créé" } th {}
                    } }
                    tbody {
                        @for d in &docs {
                            tr {
                                td class="cel-titre mono" style="font-weight:600" { (d.reference) }
                                td data-libelle="Type" { (traduire(d.type_document.cle_i18n())) }
                                td class="cel-marque" {
                                    @match d.etat {
                                        EtatDocument::Brouillon => (c::badge("alerte", traduire(d.etat.cle_i18n()))),
                                        EtatDocument::Valide => (c::badge("ok", traduire(d.etat.cle_i18n()))),
                                        EtatDocument::Abandonne => (c::badge("neutre", traduire(d.etat.cle_i18n()))),
                                    }
                                }
                                td class="num" data-libelle="Fûts" { (d.lignes) }
                                td class="petit" data-libelle=(t!("champ.client")) { (c::texte_ou_tiret(d.client.as_ref())) }
                                td class="petit discret num" data-libelle="Créé" { (c::date_heure(d.cree_le, &tz)) }
                                td class="cel-action serre" {
                                    @if d.etat == EtatDocument::Brouillon {
                                        a class="bouton bouton--petit bouton--vif"
                                          href=(format!("/operations/{}", d.id)) { (t!("action.continuer")) }
                                    } @else {
                                        a class="bouton bouton--petit"
                                          href=(format!("/documents/{}", d.id)) { (t!("action.voir")) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    let actions = html! {
        @if s.role.peut_ecrire() {
            a class="bouton bouton--vif" href="/operations/nouvelle" { (t!("nav.operation")) }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("nav.documents"))
        .actions(actions)
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

// ------------------------------------------------- étape 1 : configuration
fn etapes(courante: u8) -> Markup {
    html! {
        div class="etapes" {
            @for (n, cle) in [(1u8, t!("op.etape_configuration")), (2, t!("op.etape_capture")), (3, t!("op.etape_validation"))] {
                div class="etapes__item" aria-current=[(n == courante).then_some("step")] {
                    b { (n) } span { (cle) }
                }
            }
        }
    }
}

/// Préréglages de l'écran de configuration : les quatre gestes courants de
/// l'atelier, chacun avec son type de document, son statut cible et les
/// seuls champs qu'il exige. Ce n'est qu'un préremplissage : le serveur
/// revérifie tout à la création puis à chaque capture.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prereglage {
    Lavage,
    Remplissage,
    Livraison,
    Retour,
}

/// Champs de configuration qu'un préréglage peut mettre en avant.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ChampOp {
    Biere,
    Lot,
    Cuve,
    Emplacement,
    Client,
}

const TOUS_CHAMPS: [ChampOp; 5] = [
    ChampOp::Biere,
    ChampOp::Lot,
    ChampOp::Cuve,
    ChampOp::Emplacement,
    ChampOp::Client,
];

impl Prereglage {
    const TOUS: [Prereglage; 4] = [
        Prereglage::Lavage,
        Prereglage::Remplissage,
        Prereglage::Livraison,
        Prereglage::Retour,
    ];

    fn code(self) -> &'static str {
        match self {
            Prereglage::Lavage => "lavage",
            Prereglage::Remplissage => "remplissage",
            Prereglage::Livraison => "livraison",
            Prereglage::Retour => "retour",
        }
    }

    fn depuis_code(code: &str) -> Option<Prereglage> {
        Prereglage::TOUS.into_iter().find(|p| p.code() == code)
    }

    fn libelle(self) -> &'static str {
        match self {
            Prereglage::Lavage => t!("op.pre.lavage"),
            Prereglage::Remplissage => t!("op.pre.remplissage"),
            Prereglage::Livraison => t!("op.pre.livraison"),
            Prereglage::Retour => t!("op.pre.retour"),
        }
    }

    fn type_document(self) -> TypeDocument {
        match self {
            Prereglage::Lavage | Prereglage::Remplissage => TypeDocument::OperationInterne,
            Prereglage::Livraison => TypeDocument::Livraison,
            Prereglage::Retour => TypeDocument::Retour,
        }
    }

    fn statut(self) -> Statut {
        match self {
            Prereglage::Lavage => Statut::Propre,
            Prereglage::Remplissage => Statut::Rempli,
            Prereglage::Livraison => Statut::Livre,
            Prereglage::Retour => Statut::Recu,
        }
    }

    /// Champs affichés d'emblée — et exigés, puisque le statut cible les
    /// réclame (préconditions de `domaine::statut`).
    fn champs(self) -> &'static [ChampOp] {
        match self {
            Prereglage::Lavage | Prereglage::Retour => &[],
            Prereglage::Remplissage => &[ChampOp::Biere, ChampOp::Lot, ChampOp::Cuve],
            Prereglage::Livraison => &[ChampOp::Client],
        }
    }
}

#[derive(Deserialize)]
pub struct RequetePrereglage {
    #[serde(default)]
    prereglage: Option<String>,
}

/// Listes de référence nécessaires au formulaire de configuration.
struct Referentiels {
    bieres: Vec<persistance::referentiel::TypeBiere>,
    cuves: Vec<persistance::referentiel::Entree>,
    emplacements: Vec<persistance::referentiel::Entree>,
    clients: Vec<persistance::client::Client>,
    lots: Vec<persistance::lot::Lot>,
}

fn champ_op(champ: ChampOp, r: &Referentiels, requis: bool) -> Markup {
    match champ {
        ChampOp::Biere => c::selection(
            "type_biere_id",
            t!("champ.type_biere"),
            &c::options(&r.bieres, |x| x.id, |x| x.nom.clone()),
            None,
            Some("—"),
            requis,
        ),
        ChampOp::Lot => c::selection(
            "lot_id",
            t!("champ.lot"),
            &r.lots
                .iter()
                .map(|l| (l.id.to_string(), format!("{} — {}", l.numero, l.biere)))
                .collect::<Vec<_>>(),
            None,
            Some("—"),
            requis,
        ),
        ChampOp::Cuve => c::selection(
            "cuve_id",
            t!("champ.cuve"),
            &c::options(&r.cuves, |x| x.id, |x| x.nom.clone()),
            None,
            Some("—"),
            requis,
        ),
        ChampOp::Emplacement => c::selection(
            "emplacement_id",
            t!("champ.emplacement"),
            &c::options(&r.emplacements, |x| x.id, |x| x.nom.clone()),
            None,
            Some("—"),
            requis,
        ),
        ChampOp::Client => html! {
            div class="champ" {
                label for="champ-client_id" {
                    (t!("champ.client"))
                    @if requis { span class="discret" { " *" } }
                }
                select id="champ-client_id" name="client_id" required[requis]
                       hx-get="/clients/adresses" hx-target="#choix-adresse"
                       hx-trigger="change" hx-include="this" {
                    option value="" { "—" }
                    @for cl in &r.clients { option value=(cl.id) { (cl.societe) } }
                }
            }
            div id="choix-adresse" {
                (c::selection("adresse_client_id", t!("champ.adresse"), &[], None, Some("—"), requis))
            }
        },
    }
}

pub async fn nouvelle(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Query(q): Query<RequetePrereglage>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;

    // Reprise automatique du brouillon en cours.
    if let Some(d) = document::brouillon_en_cours(tx.conn(), s.utilisateur_id).await? {
        tx.valider().await?;
        return Ok(rediriger(
            &format!("/operations/{}", d.id),
            "ok",
            &t!("op.brouillon_repris", reference = d.reference),
            etat.config.cookies_securises,
        ));
    }

    let r = Referentiels {
        bieres: persistance::referentiel::lister_bieres(tx.conn(), true).await?,
        cuves: persistance::referentiel::lister_cuves(tx.conn(), true).await?,
        emplacements: persistance::referentiel::lister_emplacements(tx.conn(), true).await?,
        clients: persistance::client::lister(tx.conn(), false, None).await?,
        lots: persistance::lot::lister(tx.conn(), None, None, None).await?,
    };
    tx.valider().await?;

    let choix = q.prereglage.as_deref().and_then(Prereglage::depuis_code);
    let en_avant: &[ChampOp] = choix.map(|p| p.champs()).unwrap_or(&[]);
    let type_coche = choix
        .map(|p| p.type_document())
        .unwrap_or(TypeDocument::OperationInterne);
    let statut_choisi = choix.map(|p| p.statut().code());
    let bouton_lancer = html! {
        button type="submit" class="bouton bouton--vif bouton--atelier bouton--large" {
            (t!("op.lancer_capture"))
        }
    };

    let corps = html! {
        (etapes(1))
        div class="parcours" {
            @if let Some(p) = choix {
                div class="prereglage-choisi" {
                    b { (p.libelle()) }
                    a class="bouton" href="/operations/nouvelle" { (t!("op.changer")) }
                }
            } @else {
                h2 class="parcours__question" { (t!("op.que_faites_vous")) }
                div class="prereglages" {
                    @for p in Prereglage::TOUS {
                        a class="bouton bouton--atelier bouton--large"
                          href={ "/operations/nouvelle?prereglage=" (p.code()) } {
                            (p.libelle())
                        }
                    }
                }
            }
            form method="post" action="/operations" {
                (csrf(&s.jeton_csrf))
                @if let Some(p) = choix {
                    input type="hidden" name="prereglage" value=(p.code());
                    @for ch in en_avant { (champ_op(*ch, &r, true)) }
                    (bouton_lancer)
                }
                details class="plus-options" {
                    summary { (t!("op.autres_reglages")) }
                    fieldset {
                        legend { (t!("op.type_operation")) }
                        div class="choix-lignes" {
                            @for td in TypeDocument::TOUS {
                                label class="choix-ligne" {
                                    input type="radio" name="type_document" value=(td.code())
                                          checked[td == type_coche] required;
                                    span { (traduire(td.cle_i18n())) }
                                }
                            }
                        }
                    }
                    fieldset {
                        legend { (t!("op.valeurs_a_appliquer")) }
                        p class="champ__aide" style="margin-bottom:12px" { (t!("op.champs_vides")) }
                        (c::selection("statut_cible", t!("champ.statut"),
                            &Statut::TOUS.iter().map(|x| (x.code().to_string(), traduire(x.cle_i18n()).to_string())).collect::<Vec<_>>(),
                            statut_choisi, Some("—"), false))
                        @for ch in TOUS_CHAMPS {
                            @if !en_avant.contains(&ch) { (champ_op(ch, &r, false)) }
                        }
                    }
                    fieldset {
                        legend { (t!("champ.commentaire")) }
                        p class="champ__aide" { (t!("op.commentaire_avant")) }
                        (c::zone_texte("commentaire", t!("champ.commentaire"), "", None))
                    }
                    @if choix.is_none() { (bouton_lancer) }
                }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("op.titre"))
        .sous_titre(t!("op.etape_configuration"))
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct FormConfig {
    type_document: Option<String>,
    /// Préréglage d'origine : sert seulement à revenir au bon écran en cas
    /// de refus. Le serveur ne s'y fie pour rien d'autre.
    #[serde(default)]
    prereglage: Option<String>,
    #[serde(default)]
    statut_cible: Option<String>,
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
    commentaire: String,
}

impl FormConfig {
    fn configuration(&self) -> Configuration {
        Configuration {
            statut_cible: self
                .statut_cible
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(Statut::depuis_code),
            type_biere_id: uuid_opt(&self.type_biere_id),
            lot_id: uuid_opt(&self.lot_id),
            cuve_id: uuid_opt(&self.cuve_id),
            emplacement_id: uuid_opt(&self.emplacement_id),
            client_id: uuid_opt(&self.client_id),
            adresse_client_id: uuid_opt(&self.adresse_client_id),
            commentaire: self.commentaire.clone(),
        }
    }
}

pub async fn creer(
    State(etat): State<Etat>,
    Session(s): Session,
    Form(f): Form<FormConfig>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let td = f
        .type_document
        .as_deref()
        .and_then(TypeDocument::depuis_code)
        .unwrap_or(TypeDocument::OperationInterne);
    let cfg = f.configuration();
    let retour = match f.prereglage.as_deref().and_then(Prereglage::depuis_code) {
        Some(p) => format!("/operations/nouvelle?prereglage={}", p.code()),
        None => "/operations/nouvelle".to_string(),
    };

    // Préconditions du statut cible vérifiées dès la configuration : sans
    // lot ni cuve, chaque fût d'un remplissage serait refusé au scan.
    if let Some(st) = cfg.statut_cible {
        let contexte = domaine::ContexteTransition {
            type_biere_id: cfg.type_biere_id,
            lot_id: cfg.lot_id,
            cuve_id: cfg.cuve_id,
            client_id: cfg.client_id,
            adresse_client_id: cfg.adresse_client_id,
            emplacement_id: cfg.emplacement_id,
            ..Default::default()
        };
        if let Err(e) = domaine::statut::verifier_preconditions(st, &contexte) {
            return Ok(rediriger(
                &retour,
                "grave",
                &crate::i18n::message_erreur(&e),
                etat.config.cookies_securises,
            ));
        }
    }

    // Cohérence client / adresse vérifiée avant même la première capture.
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    if let (Some(cid), Some(aid)) = (cfg.client_id, cfg.adresse_client_id) {
        if !persistance::client::adresse_appartient(tx.conn(), cid, aid).await? {
            return Err(ErreurWeb::Metier(
                domaine::ErreurMetier::AdresseEtrangereAuClient,
            ));
        }
    }
    let id = document::creer(tx.conn(), td, &cfg, Some(s.utilisateur_id)).await?;
    tx.valider().await?;
    Ok(rediriger(
        &format!("/operations/{id}"),
        "ok",
        t!("op.aucune_capture"),
        etat.config.cookies_securises,
    ))
}

// ------------------------------------------------------- étape 2 : capture
pub async fn capture(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let doc = document::lire(tx.conn(), id).await?;
    if doc.etat != EtatDocument::Brouillon {
        tx.valider().await?;
        return Ok(rediriger(
            &format!("/documents/{id}"),
            "alerte",
            t!("erreur.document_non_modifiable"),
            etat.config.cookies_securises,
        ));
    }
    let lignes = document::lignes(tx.conn(), id).await?;
    tx.valider().await?;

    let n = lignes.len();
    let corps = html! {
        (etapes(2))
        (compteur_capture(n, false))
        div class="scan" {
            div {
                (ilot(&format!("/api/operations/{id}/lignes"),
                      Some("#captures"), &s.jeton_csrf, true))
            }
            aside { (resume_configuration(&doc)) }
        }
        section class="carte capture__liste" {
            div class="carte__corps carte__corps--serre" id="captures" {
                (tableau_captures(&doc, &lignes, &s.jeton_csrf))
            }
        }
        (barre_terminer(id, n, false))
        // Les gestes destructeurs sont loin du bouton principal, et chacun
        // demande une seconde confirmation explicite.
        details class="plus-options" {
            summary { (t!("op.plus_options")) }
            div class="plus-options__corps" {
                details class="confirmer" {
                    summary class="bouton bouton--large" { (t!("op.tout_retirer")) }
                    p { (t!("op.vider_confirmation", n = n)) }
                    form method="post" action=(format!("/operations/{id}/vider")) {
                        (csrf(&s.jeton_csrf))
                        button type="submit" class="bouton bouton--grave bouton--atelier bouton--large" {
                            (t!("op.oui_tout_retirer"))
                        }
                    }
                }
                details class="confirmer" {
                    summary class="bouton bouton--large" { (t!("op.abandonner")) }
                    p { (t!("op.abandonner_confirmation")) }
                    form method="post" action=(format!("/operations/{id}/abandonner")) {
                        (csrf(&s.jeton_csrf))
                        button type="submit" class="bouton bouton--grave bouton--atelier bouton--large" {
                            (t!("op.oui_abandonner"))
                        }
                    }
                }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("op.titre"))
        .sous_titre(format!(
            "{} — {}",
            doc.reference,
            traduire(doc.type_document.cle_i18n())
        ))
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

/// Compteur collé en haut de l'écran de capture. Renvoyé hors bande
/// (`hx-swap-oob`) avec chaque rafraîchissement de la liste : le chiffre
/// affiché est toujours celui du serveur, y compris après un retrait.
fn compteur_capture(n: usize, hors_bande: bool) -> Markup {
    html! {
        div id="compteur-capture" class="compteur-capture compteur-capture--collant"
            hx-swap-oob=[hors_bande.then_some("true")] aria-live="polite" {
            b id="scan-compteur" { (n) }
            span { @if n == 1 { (t!("op.compteur_un")) } @else { (t!("op.compteur_plusieurs")) } }
        }
    }
}

/// Barre d'action collée en bas, au-dessus de la barre de navigation : un
/// seul bouton, toujours au même endroit.
fn barre_terminer(id: Uuid, n: usize, hors_bande: bool) -> Markup {
    html! {
        div id="barre-terminer" class="barre-action" hx-swap-oob=[hors_bande.then_some("true")] {
            @if n == 0 {
                span class="bouton bouton--atelier bouton--large" aria-disabled="true" {
                    (t!("op.aucune_capture"))
                }
            } @else {
                a class="bouton bouton--vif bouton--atelier bouton--large"
                  href=(format!("/operations/{id}/valider")) {
                    @if n == 1 { (t!("op.terminer_un")) } @else { (t!("op.terminer", n = n)) }
                }
            }
        }
    }
}

/// Fragment renvoyé à l'écran de capture : la liste, plus le compteur et
/// le bouton « Terminer » mis à jour hors bande.
fn fragment_capture(
    d: &persistance::document::Document,
    lignes: &[persistance::document::LigneCapturee],
    jeton: &str,
) -> Markup {
    html! {
        (tableau_captures(d, lignes, jeton))
        @if d.etat == EtatDocument::Brouillon {
            (compteur_capture(lignes.len(), true))
            (barre_terminer(d.id, lignes.len(), true))
        }
    }
}

fn resume_configuration(d: &persistance::document::Document) -> Markup {
    html! {
        div class="carte" {
            div class="carte__tete" { h2 { (t!("op.recapitulatif")) } }
            div class="carte__corps" style="display:flex;flex-direction:column;gap:6px;font-size:13.5px" {
                @if let Some(st) = d.statut_cible {
                    div { span class="minus" { (t!("champ.statut")) } " " (c::badge_statut(st)) }
                }
                @if let Some(x) = &d.biere { div { span class="minus" { (t!("champ.biere")) } " " (x) } }
                @if let Some(x) = &d.lot { div { span class="minus" { (t!("champ.lot")) } " " (x) } }
                @if let Some(x) = &d.cuve { div { span class="minus" { (t!("champ.cuve")) } " " (x) } }
                @if let Some(x) = &d.emplacement { div { span class="minus" { (t!("champ.emplacement")) } " " (x) } }
                @if let Some(x) = &d.client { div { span class="minus" { (t!("champ.client")) } " " (x) } }
                @if let Some(x) = &d.adresse { div class="discret petit" { (x) } }
                @if !d.commentaire.is_empty() { div class="petit discret" { "« " (d.commentaire) " »" } }
            }
        }
    }
}

/// Tableau des fûts capturés, avec leur **état actuel avant modification**.
fn tableau_captures(
    d: &persistance::document::Document,
    lignes: &[persistance::document::LigneCapturee],
    jeton: &str,
) -> Markup {
    // Un document validé ou abandonné ne se modifie plus : pas de bouton
    // « Retirer » qui échouerait côté serveur.
    let modifiable = d.etat == EtatDocument::Brouillon;
    html! {
        @if lignes.is_empty() {
            div class="vide" { (t!("op.aucune_capture")) }
        } @else {
            div class="tableau-cadre" style="border:0" {
                table class="tableau tableau--cartes tableau--captures" {
                    thead { tr {
                        th { "#" } th { (t!("champ.code_qr")) } th { (t!("op.etat_avant")) }
                        th class="cel-secondaire" { (t!("champ.biere")) }
                        th class="cel-secondaire" { (t!("champ.client")) }
                        @if modifiable { th {} }
                    } }
                    tbody {
                        @for l in lignes {
                            tr {
                                td class="serre discret num petit" { (l.ordre) }
                                td class="cel-titre" { a class="lien-ligne" href=(format!("/futs/{}", l.fut_id)) { (l.code_qr) } }
                                td class="cel-marque" { (c::badge_statut(l.statut_avant_capture)) }
                                td class="petit cel-secondaire" data-libelle=(t!("champ.biere")) { (c::texte_ou_tiret(l.biere.as_ref())) }
                                td class="petit cel-secondaire" data-libelle=(t!("champ.client")) { (c::texte_ou_tiret(l.client.as_ref())) }
                                @if modifiable {
                                    td class="cel-action serre" {
                                        button type="button" class="bouton bouton--retirer"
                                               hx-post=(format!("/operations/{}/lignes/{}/retirer", d.id, l.id))
                                               hx-target="#captures" hx-swap="innerHTML" {
                                            (t!("action.retirer"))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        @let _ = jeton;
    }
}

pub async fn fragment_captures(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let doc = document::lire(tx.conn(), id).await?;
    let lignes = document::lignes(tx.conn(), id).await?;
    tx.valider().await?;
    Ok(fragment_capture(&doc, &lignes, &s.jeton_csrf).into_response())
}

/// Capture en rafale : aucune validation manuelle entre deux fûts.
pub async fn capturer(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
    Json(d): Json<Demande>,
) -> Response {
    if !s.role.peut_ecrire() {
        return (axum::http::StatusCode::FORBIDDEN, "interdit").into_response();
    }
    let mut tx = match etat.depot.ouvrir(s.tenant_id, Some(s.utilisateur_id)).await {
        Ok(t) => t,
        Err(e) => return echec(&d.code_qr, e.to_string()),
    };

    let doc = match document::lire(tx.conn(), id).await {
        Ok(x) => x,
        Err(e) => return echec(&d.code_qr, e.to_string()),
    };
    let resultat =
        document::capturer(tx.conn(), &doc, &d.code_qr, s.role, Some(s.utilisateur_id)).await;
    let compteur = document::lignes(tx.conn(), id)
        .await
        .map(|l| l.len() as i64)
        .ok();
    let _ = tx.valider().await;

    let rafraichir = Some(format!("/operations/{id}/lignes"));
    match resultat {
        // Le retour vert affiche en très gros la place du fût dans le lot :
        // c'est ce que l'opérateur compte en empilant.
        Ok(ResultatCapture::Ajoute(l)) => Json(Reponse {
            resultat: "succes",
            code: l.code_qr.clone(),
            titre: t!("op.numero", n = compteur.unwrap_or(l.ordre as i64)),
            detail: Some(t!("op.ajoute", code = &l.code_qr)),
            rediriger: None,
            rafraichir,
            compteur,
        })
        .into_response(),
        Ok(ResultatCapture::DejaPresent(l)) => Json(Reponse {
            resultat: "avert",
            code: l.code_qr.clone(),
            titre: t!("erreur.fut_deja_capture").into(),
            detail: Some(t!("op.deja_present", code = &l.code_qr)),
            rediriger: None,
            rafraichir: None,
            compteur,
        })
        .into_response(),
        // Refus anticipé : l'opérateur l'apprend pendant qu'il a le fût en
        // main, pas dix minutes plus tard devant l'écran de validation.
        Ok(ResultatCapture::Refuse {
            code_qr, raison, ..
        }) => Json(Reponse {
            resultat: "echec",
            code: code_qr.clone(),
            titre: code_qr.clone(),
            detail: Some(t!(
                "op.refuse",
                code = code_qr,
                raison = crate::i18n::message_erreur(&raison)
            )),
            rediriger: None,
            rafraichir: None,
            compteur,
        })
        .into_response(),
        Err(persistance::Erreur::Metier(m)) => Json(Reponse {
            resultat: "echec",
            code: d.code_qr.clone(),
            titre: t!("scan.echec").into(),
            detail: Some(crate::i18n::message_erreur(&m)),
            rediriger: None,
            rafraichir: None,
            compteur,
        })
        .into_response(),
        Err(e) => echec(&d.code_qr, e.to_string()),
    }
}

fn echec(code: &str, detail: String) -> Response {
    tracing::warn!(erreur = %detail, "capture");
    Json(Reponse {
        resultat: "echec",
        code: code.to_string(),
        titre: t!("app.erreur_serveur").into(),
        detail: None,
        rediriger: None,
        rafraichir: None,
        compteur: None,
    })
    .into_response()
}

pub async fn retirer_ligne(
    State(etat): State<Etat>,
    Session(s): Session,
    Path((id, ligne_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    document::retirer_ligne(tx.conn(), id, ligne_id).await?;
    let doc = document::lire(tx.conn(), id).await?;
    let lignes = document::lignes(tx.conn(), id).await?;
    tx.valider().await?;
    Ok(fragment_capture(&doc, &lignes, &s.jeton_csrf).into_response())
}

pub async fn vider(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    document::vider(tx.conn(), id).await?;
    tx.valider().await?;
    Ok(rediriger(
        &format!("/operations/{id}"),
        "ok",
        t!("op.aucune_capture"),
        etat.config.cookies_securises,
    ))
}

pub async fn abandonner(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    document::abandonner(tx.conn(), id).await?;
    tx.valider().await?;
    Ok(rediriger(
        "/documents",
        "ok",
        t!("document.etat.abandonne"),
        etat.config.cookies_securises,
    ))
}

// ---------------------------------------------------- étape 3 : validation
pub async fn recapitulatif(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let doc = document::lire(tx.conn(), id).await?;
    let lignes = document::lignes(tx.conn(), id).await?;
    tx.valider().await?;

    let corps = html! {
        (etapes(3))
        div style="max-width:760px;display:flex;flex-direction:column;gap:12px" {
            @if lignes.is_empty() {
                div class="avis avis--alerte" { (t!("op.rien_a_valider")) }
            } @else {
                div class="avis" style="font-size:17px" {
                    b {
                        @if let Some(st) = doc.statut_cible {
                            (t!("op.validation_resume", n = lignes.len(), statut = traduire(st.cle_i18n())))
                        } @else {
                            (t!("op.validation_resume_sans_statut", n = lignes.len()))
                        }
                    }
                }
            }
            (resume_configuration(&doc))
            section class="carte" {
                div class="carte__corps carte__corps--serre" id="captures" {
                    (tableau_captures(&doc, &lignes, &s.jeton_csrf))
                }
            }
            @if !lignes.is_empty() {
                form method="post" action=(format!("/operations/{id}/valider")) {
                    (csrf(&s.jeton_csrf))
                    button class="bouton bouton--vif bouton--atelier bouton--large" {
                        (t!("action.confirmer"))
                    }
                }
            }
            a class="bouton bouton--nu" href=(format!("/operations/{id}")) { (t!("app.retour")) }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("op.etape_validation"))
        .sous_titre(doc.reference.clone())
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

/// Application transactionnelle : tout ou rien.
///
/// Chaque transition est réévaluée — l'état a pu changer depuis la capture.
/// Une seule ligne en erreur et **rien** n'est écrit : la transaction est
/// abandonnée, le document reste un brouillon.
pub async fn valider(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let securise = etat.config.cookies_securises;
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;

    match document::valider(&mut tx, s.role, id).await {
        Ok(rapport) => {
            persistance::audit::tracer(
                tx.conn(),
                Some(s.utilisateur_id),
                "document.valide",
                Some("document"),
                Some(&id.to_string()),
                serde_json::json!({ "reference": rapport.reference, "futs": rapport.futs }),
                None,
            )
            .await?;
            tx.valider().await?;
            Ok(rediriger(
                &format!("/documents/{id}"),
                "ok",
                &t!("op.valide", reference = rapport.reference, n = rapport.futs),
                securise,
            ))
        }
        Err(e) => {
            // La transaction n'est jamais validée : aucune écriture ne subsiste.
            drop(tx);
            let message = match &e {
                persistance::Erreur::Metier(m) => crate::i18n::message_erreur(m),
                autre => {
                    tracing::error!(erreur = %autre, "validation de document");
                    t!("app.erreur_serveur").to_string()
                }
            };
            Ok(rediriger(
                &format!("/operations/{id}/valider"),
                "grave",
                &message,
                securise,
            ))
        }
    }
}

// ------------------------------------------------------------- consultation
pub async fn fiche(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let doc = document::lire(tx.conn(), id).await?;
    let lignes = document::lignes(tx.conn(), id).await?;
    tx.valider().await?;
    let tz = s.fuseau_horaire.clone();

    let corps = html! {
        div class="duo duo--etroit" {
            section class="carte" {
                div class="carte__tete" {
                    h2 { (lignes.len()) " fûts" }
                    @match doc.etat {
                        EtatDocument::Valide => (c::badge("ok", traduire(doc.etat.cle_i18n()))),
                        EtatDocument::Brouillon => (c::badge("alerte", traduire(doc.etat.cle_i18n()))),
                        EtatDocument::Abandonne => (c::badge("neutre", traduire(doc.etat.cle_i18n()))),
                    }
                }
                div class="carte__corps carte__corps--serre" {
                    (tableau_captures(&doc, &lignes, &s.jeton_csrf))
                }
            }
            div class="duo__col" {
                (resume_configuration(&doc))
                div class="carte" { div class="carte__corps" style="font-size:13.5px;display:flex;flex-direction:column;gap:5px" {
                    div { span class="minus" { "Créé" } " " (c::date_heure(doc.cree_le, &tz))
                          @if let Some(q) = &doc.cree_par_nom { span class="discret" { " — " (q) } } }
                    @if let Some(v) = doc.valide_le {
                        div { span class="minus" { "Validé" } " " (c::date_heure(v, &tz))
                              @if let Some(q) = &doc.valide_par_nom { span class="discret" { " — " (q) } } }
                    }
                } }
                @if doc.type_document == TypeDocument::Livraison && doc.etat == EtatDocument::Valide {
                    a class="bouton bouton--vif bouton--large"
                      href=(format!("/documents/{id}/bon-de-livraison")) target="_blank" {
                        (t!("op.bon_de_livraison"))
                    }
                }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, doc.reference.clone())
        .sous_titre(traduire(doc.type_document.cle_i18n()))
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct RequeteImpression {
    #[serde(default)]
    pub auto: Option<u8>,
}

/// Bon de livraison : une page HTML avec une feuille d'impression soignée.
/// Aucune dépendance PDF en v1 — on le fait signer au client à la livraison.
pub async fn bon_de_livraison(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
    Query(_q): Query<RequeteImpression>,
) -> Result<Response, ErreurWeb> {
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let doc = document::lire(tx.conn(), id).await?;
    let lignes = document::lignes(tx.conn(), id).await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    tx.valider().await?;
    let tz = s.fuseau_horaire.clone();

    let total_litres: i32 = lignes.iter().map(|l| l.contenance_litres).sum();
    let corps = html! {
        div class="bon" {
            div class="bon__tete" {
                div {
                    h1 style="margin:0 0 4px" { (loc.nom) }
                    p style="margin:0;font-size:11pt" { (t!("op.bon_de_livraison")) }
                }
                div style="text-align:right;font-size:11pt" {
                    div { b { (doc.reference) } }
                    @if let Some(v) = doc.valide_le { div { (c::date_heure(v, &tz)) } }
                }
            }
            div style="display:grid;grid-template-columns:1fr 1fr;gap:20px;margin-bottom:14px;font-size:11pt" {
                div {
                    div class="minus" { (t!("champ.client")) }
                    div { b { (doc.client.clone().unwrap_or_default()) } }
                    div { (doc.adresse.clone().unwrap_or_default()) }
                }
                div {
                    div class="minus" { "Contenu" }
                    @if let Some(b) = &doc.biere { div { (b) } }
                    @if let Some(l) = &doc.lot { div { (t!("champ.lot")) " " (l) } }
                }
            }
            table class="tableau" style="width:100%;font-size:10.5pt" {
                thead { tr {
                    th { "#" } th { (t!("champ.code_qr")) } th { (t!("champ.contenance")) }
                    th { (t!("champ.biere")) } th { (t!("champ.lot")) }
                } }
                tbody {
                    @for (i, l) in lignes.iter().enumerate() {
                        tr {
                            td { (i + 1) }
                            td class="mono" { (l.code_qr) }
                            td { (l.contenance_litres) " L" }
                            td { (l.biere.clone().unwrap_or_default()) }
                            td { (l.lot.clone().unwrap_or_default()) }
                        }
                    }
                }
                tfoot { tr {
                    td colspan="2" { b { (lignes.len()) " fûts" } }
                    td colspan="3" { b { (total_litres) " L" } }
                } }
            }
            @if !doc.commentaire.is_empty() {
                p style="margin-top:12px;font-size:10.5pt" { (doc.commentaire) }
            }
            div class="bon__signatures" {
                div class="bon__signature" { (t!("op.signature_livreur")) }
                div class="bon__signature" { (t!("op.signature_client")) }
            }
            p class="sans-impression" style="margin-top:20px" {
                button class="bouton bouton--vif" data-imprimer { (t!("app.imprimer")) }
            }
        }
    };
    Ok(page_nue(&doc.reference, corps).into_response())
}
