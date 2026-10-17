use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Form;
use maud::html;
use serde::Deserialize;
use uuid::Uuid;

use domaine::statut::Statut;

use crate::gabarits::composants as c;
use crate::gabarits::csrf;
use crate::gabarits::socle::{page, Cadre};
use crate::middleware::Session;
use crate::routes::{page_avec_avis, rediriger, Contexte};
use crate::{t, ErreurWeb, Etat};

#[derive(Deserialize, Default)]
pub struct Requete {
    q: Option<String>,
    biere: Option<String>,
    dluo: Option<u8>,
    nouveau: Option<u8>,
}

pub async fn liste(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Query(r): Query<Requete>,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    let biere_filtre = r.biere.as_deref().and_then(|b| Uuid::parse_str(b).ok());
    let dluo_alerte = r.dluo == Some(1);
    let lots = persistance::lot::lister(
        tx.conn(),
        r.q.as_deref().filter(|x| !x.trim().is_empty()),
        biere_filtre,
        dluo_alerte.then_some(loc.seuil_alerte_dluo_jours),
    )
    .await?;
    let bieres = persistance::referentiel::lister_bieres(tx.conn(), true).await?;
    let cuves = persistance::referentiel::lister_cuves(tx.conn(), true).await?;
    tx.valider().await?;

    let aujourdhui = chrono::Utc::now().date_naive();
    let peut = s.role.peut_ecrire();
    let formulaire_ouvert = peut && r.nouveau == Some(1);
    let corps = html! {
        @if formulaire_ouvert {
            section class="carte" id="nouveau-lot" style="margin-bottom:14px;max-width:700px" {
                div class="carte__tete" { h2 { (t!("lot.nouveau")) } }
                div class="carte__corps" {
                    form method="post" action="/lots" {
                        (csrf(&s.jeton_csrf))
                        div class="grille" {
                            (c::Champ::texte("numero", t!("champ.numero"), "").requis().rendre())
                            (c::Champ::texte("date_brassage", t!("champ.date_brassage"),
                                aujourdhui.to_string()).de_type("date").requis().rendre())
                            (c::selection("type_biere_id", t!("champ.type_biere"),
                                &c::options(&bieres, |x| x.id, |x| x.nom.clone()),
                                None, Some("—"), true))
                            (c::selection("cuve_id", t!("champ.cuve"),
                                &c::options(&cuves, |x| x.id, |x| x.nom.clone()),
                                None, Some("—"), true))
                            (c::Champ::texte("dluo", t!("champ.dluo"), "").de_type("date").rendre())
                        }
                        (c::zone_texte("commentaire", t!("champ.commentaire"), "", None))
                        button class="bouton bouton--vif" { (t!("action.creer")) }
                    }
                }
            }
        }
        form class="filtres" method="get" action="/lots" {
            div class="champ champ--large" {
                label for="champ-q" { (t!("action.rechercher")) }
                input id="champ-q" type="search" name="q" value=[r.q.as_deref()]
                      placeholder=(t!("champ.numero"));
            }
            (c::selection("biere", t!("champ.biere"),
                &c::options(&bieres, |x| x.id, |x| x.nom.clone()),
                biere_filtre.map(|b| b.to_string()).as_deref(), Some(t!("lot.toutes_bieres")), false))
            div class="champ" {
                label class="coche" {
                    input type="checkbox" name="dluo" value="1" checked[dluo_alerte];
                    span { (t!("lot.dluo_alerte")) }
                }
            }
            button class="bouton" { (t!("action.filtrer")) }
        }
        @if lots.is_empty() {
            (c::vide(
                t!("lot.aucun"),
                peut.then(|| html! {
                    a class="bouton bouton--vif" href="/lots?nouveau=1#nouveau-lot" { (t!("lot.nouveau")) }
                }),
            ))
        } @else {
            div class="tableau-cadre" {
                table class="tableau tableau--cartes" {
                    thead { tr {
                        th { (t!("champ.numero")) } th { (t!("champ.biere")) } th { (t!("champ.cuve")) }
                        th { (t!("champ.date_brassage")) } th { (t!("champ.dluo")) }
                        th { (t!("lot.futs_rattaches")) } th {}
                    } }
                    tbody {
                        @for l in &lots {
                            tr {
                                td class="cel-titre mono" style="font-weight:600" {
                                    a href=(format!("/lots/{}", l.id)) { (l.numero) }
                                }
                                td data-libelle=(t!("champ.biere")) { span style="display:inline-flex;align-items:center;gap:6px" {
                                    span class="pastille" style=(format!("background:{}", l.couleur_biere)) {}
                                    (l.biere) } }
                                td class="petit" data-libelle=(t!("champ.cuve")) { (l.cuve) }
                                td class="num petit" data-libelle=(t!("champ.date_brassage")) { (c::jour(l.date_brassage)) }
                                td class="cel-marque num" {
                                    @match l.dluo {
                                        Some(d) => {
                                            @let reste = (d - aujourdhui).num_days();
                                            @if reste < 0 { (c::badge("grave", &c::jour(d))) }
                                            @else if reste < loc.seuil_alerte_dluo_jours as i64 {
                                                (c::badge("alerte", &c::jour(d)))
                                            } @else { (c::jour(d)) }
                                        },
                                        None => span class="discret" { "—" },
                                    }
                                }
                                td class="num" data-libelle=(t!("lot.futs_rattaches")) {
                                    @if l.futs > 0 {
                                        a href=(format!("/parc?lot={}", l.id)) { (l.futs) }
                                    } @else { span class="discret" { "0" } }
                                }
                                td class="cel-action serre boutons" {
                                    a class="bouton bouton--petit" href=(format!("/lots/{}/rappel", l.id)) {
                                        (t!("rappel.titre"))
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
        @if peut {
            a class="bouton bouton--vif" href="/lots?nouveau=1#nouveau-lot" { (t!("lot.nouveau")) }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("lot.titre"))
        .actions(actions)
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct FormLot {
    numero: String,
    date_brassage: String,
    type_biere_id: Uuid,
    cuve_id: Uuid,
    #[serde(default)]
    dluo: String,
    #[serde(default)]
    commentaire: String,
}

pub async fn creer(
    State(etat): State<Etat>,
    Session(s): Session,
    Form(f): Form<FormLot>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let brassage = chrono::NaiveDate::parse_from_str(&f.date_brassage, "%Y-%m-%d")
        .map_err(|_| ErreurWeb::champ("date_brassage", t!("erreur.date_invalide")))?;
    let dluo = if f.dluo.trim().is_empty() {
        None
    } else {
        Some(
            chrono::NaiveDate::parse_from_str(f.dluo.trim(), "%Y-%m-%d")
                .map_err(|_| ErreurWeb::champ("dluo", t!("erreur.date_invalide")))?,
        )
    };
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let resultat = persistance::lot::creer(
        tx.conn(),
        &persistance::lot::DonneesLot {
            numero: &f.numero,
            date_brassage: brassage,
            type_biere_id: f.type_biere_id,
            cuve_id: f.cuve_id,
            dluo,
            commentaire: &f.commentaire,
        },
    )
    .await;
    match resultat {
        Ok(id) => {
            tx.valider().await?;
            Ok(rediriger(
                &format!("/lots/{id}"),
                "ok",
                t!("lot.enregistre"),
                securise,
            ))
        }
        Err(persistance::Erreur::Conflit(_)) => Ok(rediriger(
            "/lots",
            "grave",
            t!("erreur.deja_utilise"),
            securise,
        )),
        Err(e) => Err(e.into()),
    }
}

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
    let lot = persistance::lot::lire(tx.conn(), id).await?;
    let par_statut = persistance::lot::futs_par_statut(tx.conn(), id).await?;
    let detenteurs = persistance::lot::detenteurs(tx.conn(), id).await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    let bieres = persistance::referentiel::lister_bieres(tx.conn(), true).await?;
    tx.valider().await?;

    let aujourdhui = chrono::Utc::now().date_naive();
    let peut = s.role.peut_ecrire();
    let verrouille = lot.futs > 0;
    let dluo_txt = lot.dluo.map(|d| d.to_string()).unwrap_or_default();

    let corps = html! {
        div class="cartes" style="margin-bottom:14px" {
            div class="tuile" {
                span class="tuile__valeur num" { (lot.futs) }
                span class="tuile__nom" { (t!("lot.futs_rattaches")) }
            }
            div class="tuile" {
                span class="tuile__nom" { (t!("champ.dluo")) }
                span class="tuile__valeur num" {
                    @match lot.dluo {
                        Some(d) => {
                            @let reste = (d - aujourdhui).num_days();
                            @if reste < 0 {
                                (c::badge("grave", &format!("{} — {}", c::jour(d), t!("lot.dluo_depassee"))))
                            } @else if reste < loc.seuil_alerte_dluo_jours as i64 {
                                (c::badge("alerte", &format!("{} — {}", c::jour(d), t!("lot.dluo_proche", n = reste))))
                            } @else { (c::jour(d)) }
                        },
                        None => span class="discret" { "—" },
                    }
                }
            }
        }
        section class="carte" style="margin-bottom:14px;max-width:700px" {
            div class="carte__tete" { h2 { (t!("lot.fiche")) } }
            div class="carte__corps" {
                form method="post" action=(format!("/lots/{id}")) {
                    (csrf(&s.jeton_csrf))
                    @if verrouille {
                        p class="petit discret" { (t!("lot.verrouille")) }
                    }
                    div class="grille" {
                        @if verrouille || !peut {
                            div class="champ" {
                                label { (t!("champ.numero")) }
                                input class="mono" value=(lot.numero) readonly;
                            }
                            div class="champ" {
                                label { (t!("champ.biere")) }
                                input value=(lot.biere) readonly;
                            }
                        } @else {
                            (c::Champ::texte("numero", t!("champ.numero"), lot.numero.clone()).requis().rendre())
                            (c::selection("type_biere_id", t!("champ.type_biere"),
                                &c::options(&bieres, |x| x.id, |x| x.nom.clone()),
                                Some(lot.type_biere_id.to_string().as_str()), Some("—"), true))
                        }
                        div class="champ" {
                            label { (t!("champ.cuve")) }
                            input value=(lot.cuve) readonly;
                        }
                        div class="champ" {
                            label { (t!("champ.date_brassage")) }
                            input value=(c::jour(lot.date_brassage)) readonly;
                        }
                        @if peut {
                            (c::Champ::texte("dluo", t!("champ.dluo"), dluo_txt).de_type("date").rendre())
                        }
                    }
                    @if peut {
                        (c::zone_texte("commentaire", t!("champ.commentaire"), &lot.commentaire, None))
                        button class="bouton bouton--vif" { (t!("action.enregistrer")) }
                    } @else if !lot.commentaire.is_empty() {
                        p { (lot.commentaire) }
                    }
                }
            }
        }
        h2 { (t!("lot.futs_par_statut")) }
        @if par_statut.is_empty() {
            (c::vide(t!("lot.aucun_fut"), None))
        } @else {
            div class="cartes" style="margin-bottom:14px" {
                @for st in Statut::TOUS {
                    @if let Some((_, n)) = par_statut.iter().find(|(x, _)| *x == st) {
                        div class="tuile" {
                            span class="tuile__valeur num" {
                                a href=(format!("/parc?lot={id}&statut={}", st.code())) { (n) }
                            }
                            span class="tuile__nom" { (c::badge_statut(st)) }
                        }
                    }
                }
            }
        }
        h2 { (t!("lot.detenteurs")) }
        @if detenteurs.is_empty() {
            (c::vide(t!("lot.aucun_detenteur"), None))
        } @else {
            div class="tableau-cadre" {
                table class="tableau" {
                    thead { tr { th { (t!("champ.societe")) } th { (t!("rappel.futs")) } } }
                    tbody {
                        @for d in &detenteurs {
                            tr {
                                td style="font-weight:600" {
                                    a href=(format!("/clients/{}", d.client_id)) { (d.societe) }
                                }
                                td class="num" { (d.futs) }
                            }
                        }
                    }
                }
            }
        }
    };
    let actions = html! {
        a class="bouton" href=(format!("/lots/{id}/rappel")) { (t!("rappel.titre")) }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("lot.fiche"))
        .sous_titre(format!("{} — {}", lot.numero, lot.biere))
        .actions(actions)
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct FormModif {
    numero: Option<String>,
    type_biere_id: Option<Uuid>,
    #[serde(default)]
    dluo: String,
    #[serde(default)]
    commentaire: String,
}

pub async fn modifier(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
    Form(f): Form<FormModif>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let dluo = if f.dluo.trim().is_empty() {
        None
    } else {
        Some(
            chrono::NaiveDate::parse_from_str(f.dluo.trim(), "%Y-%m-%d")
                .map_err(|_| ErreurWeb::champ("dluo", t!("erreur.date_invalide")))?,
        )
    };
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let lot = persistance::lot::lire(tx.conn(), id).await?;
    // Numéro et bière : figés dès qu'un fût est rattaché, quoi que dise le formulaire.
    let (numero, biere) = if lot.futs > 0 {
        (lot.numero.clone(), lot.type_biere_id)
    } else {
        (
            f.numero.unwrap_or_else(|| lot.numero.clone()),
            f.type_biere_id.unwrap_or(lot.type_biere_id),
        )
    };
    if numero.trim().is_empty() {
        return Err(ErreurWeb::champ("numero", t!("erreur.champ_requis")));
    }
    let resultat = persistance::lot::modifier(
        tx.conn(),
        id,
        &persistance::lot::DonneesLot {
            numero: &numero,
            date_brassage: lot.date_brassage,
            type_biere_id: biere,
            cuve_id: lot.cuve_id,
            dluo,
            commentaire: &f.commentaire,
        },
    )
    .await;
    let url = format!("/lots/{id}");
    match resultat {
        Ok(()) => {
            tx.valider().await?;
            Ok(rediriger(&url, "ok", t!("lot.modifie"), securise))
        }
        Err(persistance::Erreur::Conflit(_)) => Ok(rediriger(
            &url,
            "grave",
            t!("erreur.deja_utilise"),
            securise,
        )),
        Err(e) => Err(e.into()),
    }
}

/// Rappel produit : à partir d'un lot, tous les clients qui l'ont reçu.
///
/// C'est une obligation légale en agroalimentaire, et c'est pourquoi c'est
/// une fonctionnalité nommée avec son propre écran, pas un filtre perdu
/// dans une liste.
pub async fn rappel(
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
    let lot = persistance::lot::lire(tx.conn(), id).await?;
    let destinataires = persistance::lot::rappel_produit(tx.conn(), id).await?;
    tx.valider().await?;
    let tz = s.fuseau_horaire.clone();
    let total_futs: i64 = destinataires.iter().map(|d| d.futs).sum();
    let total_detenus: i64 = destinataires.iter().map(|d| d.futs_encore_detenus).sum();

    let corps = html! {
        div class="cartes" style="margin-bottom:14px" {
            div class="tuile" {
                span class="tuile__valeur num" { (destinataires.len()) }
                span class="tuile__nom" { (t!("rappel.clients_concernes")) }
            }
            div class="tuile" {
                span class="tuile__valeur num" { (total_futs) }
                span class="tuile__nom" { (t!("rappel.futs_livres")) }
            }
            div class={ "tuile " @if total_detenus > 0 { "tuile--grave" } @else { "tuile--ok" } } {
                span class="tuile__valeur num" { (total_detenus) }
                span class="tuile__nom" { (t!("rappel.encore_detenus")) }
            }
        }
        @if destinataires.is_empty() {
            (c::vide(t!("rappel.aucun"), None))
        } @else {
            p { (t!("rappel.resultat", n = destinataires.len(), lot = &lot.numero)) }
            div class="tableau-cadre" {
                table class="tableau" {
                    thead { tr {
                        th { (t!("champ.societe")) } th { (t!("champ.contact")) }
                        th { (t!("champ.telephone")) } th { (t!("champ.adresse")) }
                        th { (t!("rappel.futs")) } th { (t!("rappel.encore_detenus")) }
                        th { (t!("rappel.derniere_livraison")) } th { (t!("champ.code_qr")) }
                    } }
                    tbody {
                        @for d in &destinataires {
                            tr {
                                td style="font-weight:600" {
                                    a href=(format!("/clients/{}", d.client_id)) { (d.societe) }
                                }
                                td class="petit" { (d.contact_nom) }
                                td class="petit mono" { (d.telephone) }
                                td class="petit discret" { (d.adresse) }
                                td class="num" { (d.futs) }
                                td class="num" {
                                    @if d.futs_encore_detenus > 0 {
                                        (c::badge("grave", &d.futs_encore_detenus.to_string()))
                                    } @else { span class="discret" { "0" } }
                                }
                                td class="petit num discret" { (c::date_heure(d.derniere_livraison, &tz)) }
                                td class="petit mono discret" style="max-width:280px" {
                                    (d.codes_qr.join(" "))
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    let actions = html! {
        a class="bouton" href=(format!("/lots/{id}/rappel/export")) { (t!("rappel.exporter")) }
        button class="bouton sans-impression" data-imprimer { (t!("app.imprimer")) }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("rappel.titre"))
        .sous_titre(format!(
            "{} — {} — {}",
            lot.numero,
            lot.biere,
            t!("rappel.brasse_le", date = c::jour(lot.date_brassage))
        ))
        .actions(actions)
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

pub async fn rappel_export(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
) -> Result<Response, ErreurWeb> {
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let lot = persistance::lot::lire(tx.conn(), id).await?;
    let destinataires = persistance::lot::rappel_produit(tx.conn(), id).await?;
    tx.valider().await?;

    let mut w = csv::WriterBuilder::new()
        .delimiter(b';')
        .from_writer(vec![]);
    w.write_record([
        t!("champ.societe"),
        t!("champ.contact"),
        t!("champ.telephone"),
        t!("champ.email"),
        t!("champ.adresse"),
        t!("rappel.futs"),
        t!("rappel.encore_detenus"),
        t!("rappel.premiere_livraison"),
        t!("rappel.derniere_livraison"),
        t!("champ.code_qr"),
    ])
    .ok();
    for d in &destinataires {
        w.write_record([
            d.societe.clone(),
            d.contact_nom.clone(),
            d.telephone.clone(),
            d.email.clone(),
            d.adresse.clone(),
            d.futs.to_string(),
            d.futs_encore_detenus.to_string(),
            c::date_heure(d.premiere_livraison, &s.fuseau_horaire),
            c::date_heure(d.derniere_livraison, &s.fuseau_horaire),
            d.codes_qr.join(" "),
        ])
        .ok();
    }
    let mut corps = String::from("\u{feff}");
    corps.push_str(&String::from_utf8_lossy(
        &w.into_inner()
            .map_err(|e| ErreurWeb::Interne(e.to_string()))?,
    ));
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"rappel-{}.csv\"", lot.numero),
            ),
        ],
        corps,
    )
        .into_response())
}
