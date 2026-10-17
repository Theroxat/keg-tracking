use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::Form;
use maud::html;
use serde::Deserialize;
use uuid::Uuid;

use crate::gabarits::composants as c;
use crate::gabarits::csrf;
use crate::gabarits::socle::{page, Cadre};
use crate::middleware::Session;
use crate::routes::{page_avec_avis, rediriger, Contexte};
use crate::{t, ErreurWeb, Etat};

#[derive(Deserialize, Default)]
pub struct Requete {
    q: Option<String>,
    archives: Option<u8>,
}

pub async fn liste(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Query(r): Query<Requete>,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis.is_some();
    let inclure = r.archives == Some(1);
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let clients = persistance::client::lister(
        tx.conn(),
        inclure,
        r.q.as_deref().filter(|x| !x.trim().is_empty()),
    )
    .await?;
    tx.valider().await?;

    let corps = html! {
        form class="filtres" method="get" action="/clients" {
            div class="champ champ--large" {
                label for="champ-q" { (t!("action.rechercher")) }
                input id="champ-q" type="search" name="q" value=[r.q.as_deref()]
                      placeholder=(t!("champ.societe"));
            }
            div class="champ" {
                label class="coche" {
                    input type="checkbox" name="archives" value="1" checked[inclure];
                    span { (t!("client.archives_inclus")) }
                }
            }
            button type="submit" class="bouton" { (t!("action.filtrer")) }
        }
        @if clients.is_empty() {
            (c::vide(
                t!("app.aucun_resultat"),
                s.role.peut_ecrire().then(|| html! {
                    a class="bouton bouton--vif" href="/clients/nouveau" { (t!("client.creer")) }
                }),
            ))
        } @else {
            div class="tableau-cadre" {
                table class="tableau tableau--cartes" {
                    thead { tr {
                        th { (t!("champ.societe")) } th { (t!("champ.contact")) }
                        th { (t!("champ.telephone")) } th { (t!("client.adresses")) }
                        th { (t!("client.futs_detenus")) } th {}
                    } }
                    tbody {
                        @for cl in &clients {
                            tr {
                                td class="cel-titre" {
                                    a href=(format!("/clients/{}", cl.id)) style="font-weight:600" { (cl.societe) }
                                    @if cl.archive { " " (c::badge("neutre", t!("app.archive"))) }
                                }
                                td class="petit" data-libelle=(t!("champ.contact")) {
                                    (format!("{} {}", cl.contact_prenom, cl.contact_nom).trim())
                                }
                                td class="petit mono" data-libelle=(t!("champ.telephone")) { (cl.telephone) }
                                td class="num petit discret" data-libelle=(t!("client.adresses")) { (cl.adresses) }
                                td class="cel-marque num" {
                                    @if cl.futs_detenus > 0 {
                                        a href=(format!("/parc?client={}&statut=livre", cl.id)) {
                                            (c::badge("livre", &cl.futs_detenus.to_string()))
                                        }
                                    } @else { span class="discret" { "0" } }
                                }
                                td class="cel-action serre" {
                                    a class="bouton bouton--petit" href=(format!("/clients/{}", cl.id)) {
                                        (t!("action.voir"))
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
            a class="bouton bouton--vif" href="/clients/nouveau" { (t!("client.nouveau")) }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("client.titre"))
        .sous_titre(if clients.len() == 1 {
            t!("client.un").to_string()
        } else {
            t!("client.nombre", n = clients.len())
        })
        .actions(actions)
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

pub async fn formulaire_nouveau(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let corps = html! {
        form method="post" action="/clients" style="max-width:720px" {
            (csrf(&s.jeton_csrf))
            fieldset {
                legend { (t!("client.fiche")) }
                (c::Champ::texte("societe", t!("champ.societe"), "").requis().rendre())
                div class="grille" {
                    (c::Champ::texte("contact_prenom", t!("champ.prenom"), "").rendre())
                    (c::Champ::texte("contact_nom", t!("champ.nom"), "").rendre())
                    (c::Champ::texte("telephone", t!("champ.telephone"), "").de_type("tel").rendre())
                    (c::Champ::texte("email", t!("champ.email"), "").de_type("email").rendre())
                }
                (c::zone_texte("commentaire", t!("champ.commentaire"), "", None))
            }
            // La première adresse se saisit dans le même formulaire : un
            // client sans adresse ne peut rien recevoir.
            fieldset {
                legend { (t!("client.premiere_adresse")) }
                (c::Champ::texte("libelle", t!("champ.libelle"), "Principale").rendre())
                (c::Champ::texte("rue", t!("champ.rue"), "").rendre())
                div class="grille" {
                    (c::Champ::texte("code_postal", t!("champ.code_postal"), "").rendre())
                    (c::Champ::texte("ville", t!("champ.ville"), "").rendre())
                }
            }
            div class="boutons" {
                button type="submit" class="bouton bouton--vif" { (t!("action.creer")) }
                a class="bouton bouton--nu" href="/clients" { (t!("action.annuler")) }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("client.nouveau")).avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        false,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct FormClient {
    societe: String,
    #[serde(default)]
    contact_nom: String,
    #[serde(default)]
    contact_prenom: String,
    #[serde(default)]
    telephone: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    commentaire: String,
    #[serde(default)]
    libelle: String,
    #[serde(default)]
    rue: String,
    #[serde(default)]
    code_postal: String,
    #[serde(default)]
    ville: String,
}

pub async fn creer(
    State(etat): State<Etat>,
    Session(s): Session,
    Form(f): Form<FormClient>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    if f.societe.trim().is_empty() {
        return Err(ErreurWeb::champ("societe", t!("erreur.champ_requis")));
    }
    let securise = etat.config.cookies_securises;
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let id = persistance::client::creer(
        tx.conn(),
        &persistance::client::DonneesClient {
            societe: &f.societe,
            contact_nom: &f.contact_nom,
            contact_prenom: &f.contact_prenom,
            telephone: &f.telephone,
            email: &f.email,
            commentaire: &f.commentaire,
        },
    )
    .await?;
    persistance::client::creer_adresse(
        tx.conn(),
        id,
        &persistance::client::DonneesAdresse {
            libelle: &f.libelle,
            rue: &f.rue,
            code_postal: &f.code_postal,
            ville: &f.ville,
        },
    )
    .await?;
    persistance::audit::tracer(
        tx.conn(),
        Some(s.utilisateur_id),
        "client.cree",
        Some("client"),
        Some(&id.to_string()),
        serde_json::json!({ "societe": f.societe }),
        None,
    )
    .await?;
    tx.valider().await?;
    Ok(rediriger(
        &format!("/clients/{id}"),
        "ok",
        t!("client.enregistre"),
        securise,
    ))
}

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
    let cl = persistance::client::lire(tx.conn(), id).await?;
    let adresses = persistance::client::lister_adresses(tx.conn(), id, true).await?;
    let futs = persistance::fut::detenus_par_client(tx.conn(), id).await?;
    tx.valider().await?;

    let peut = s.role.peut_ecrire();
    let corps = html! {
        div class="duo duo--large" {
            div class="duo__col" {
                section class="carte" {
                    div class="carte__tete" {
                        h2 { (t!("client.futs_detenus")) }
                        span class="badge badge--livre" { (futs.len()) }
                    }
                    div class="carte__corps carte__corps--serre" {
                        @if futs.is_empty() {
                            div class="vide" { (t!("client.aucun_fut")) }
                        } @else {
                            div class="tableau-cadre" style="border:0" {
                                table class="tableau tableau--cartes" {
                                    thead { tr {
                                        th { (t!("champ.code_qr")) } th { (t!("champ.lot")) }
                                        th { (t!("champ.anciennete")) }
                                    } }
                                    tbody {
                                        @for f in &futs {
                                            tr {
                                                td class="cel-titre" { a class="lien-ligne" href=(format!("/futs/{}", f.id)) { (f.code_qr) } }
                                                td class="mono petit" data-libelle=(t!("champ.lot")) { (c::texte_ou_tiret(f.lot.as_ref())) }
                                                td class="cel-marque serre num" {
                                                    @if f.jours > 30 { (c::badge("grave", &format!("{} {}", f.jours, t!("unite.jours_court")))) }
                                                    @else { span class="petit" { (f.jours) " j" } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                section class="carte" {
                    div class="carte__tete" { h2 { (t!("client.adresses")) } }
                    div class="carte__corps" {
                        @for a in &adresses {
                            form method="post" action=(format!("/clients/{}/adresses/{}", id, a.id))
                                 style="border-bottom:1px solid var(--bordure);padding-bottom:12px;margin-bottom:12px" {
                                (csrf(&s.jeton_csrf))
                                div class="grille" {
                                    (c::Champ::texte("libelle", t!("champ.libelle"), &a.libelle).rendre())
                                    (c::Champ::texte("rue", t!("champ.rue"), &a.rue).rendre())
                                    (c::Champ::texte("code_postal", t!("champ.code_postal"), &a.code_postal).rendre())
                                    (c::Champ::texte("ville", t!("champ.ville"), &a.ville).rendre())
                                }
                                @if peut {
                                    div class="boutons" {
                                        button class="bouton bouton--petit" { (t!("action.enregistrer")) }
                                        button class="bouton bouton--petit bouton--nu" name="archive"
                                               value=(if a.archive { "0" } else { "1" }) {
                                            @if a.archive { (t!("action.desarchiver")) } @else { (t!("action.archiver")) }
                                        }
                                        @if a.archive { (c::badge("neutre", t!("app.archivee"))) }
                                    }
                                }
                            }
                        }
                        @if peut {
                            details {
                                summary class="bouton bouton--petit" { (t!("client.nouvelle_adresse")) }
                                form method="post" action=(format!("/clients/{id}/adresses")) style="padding-top:12px" {
                                    (csrf(&s.jeton_csrf))
                                    div class="grille" {
                                        (c::Champ::texte("libelle", t!("champ.libelle"), "").rendre())
                                        (c::Champ::texte("rue", t!("champ.rue"), "").rendre())
                                        (c::Champ::texte("code_postal", t!("champ.code_postal"), "").rendre())
                                        (c::Champ::texte("ville", t!("champ.ville"), "").rendre())
                                    }
                                    button class="bouton bouton--vif bouton--petit" { (t!("action.ajouter")) }
                                }
                            }
                        }
                    }
                }
            }

            div class="duo__col sans-impression" {
                section class="carte" {
                    div class="carte__tete" { h2 { (t!("client.fiche")) } }
                    div class="carte__corps" {
                        form method="post" action=(format!("/clients/{id}")) {
                            (csrf(&s.jeton_csrf))
                            (c::Champ::texte("societe", t!("champ.societe"), &cl.societe).requis().rendre())
                            (c::Champ::texte("contact_prenom", t!("champ.prenom"), &cl.contact_prenom).rendre())
                            (c::Champ::texte("contact_nom", t!("champ.nom"), &cl.contact_nom).rendre())
                            (c::Champ::texte("telephone", t!("champ.telephone"), &cl.telephone).de_type("tel").rendre())
                            (c::Champ::texte("email", t!("champ.email"), &cl.email).de_type("email").rendre())
                            (c::zone_texte("commentaire", t!("champ.commentaire"), &cl.commentaire, None))
                            @if peut { button class="bouton bouton--vif bouton--large" { (t!("action.enregistrer")) } }
                        }
                    }
                }
                @if peut {
                    section class="carte" {
                        div class="carte__corps" {
                            p class="petit discret" { (t!("client.suppression_impossible")) }
                            @if cl.futs_detenus > 0 && !cl.archive {
                                div class="avis avis--alerte" style="margin:0" {
                                    (t!("client.archive_impossible", n = cl.futs_detenus))
                                }
                            } @else {
                                form method="post" action=(format!("/clients/{id}/archiver"))
                                      data-confirmer=[(!cl.archive).then(|| c::confirmer_soumission(t!("client.archiver_confirmation")))] {
                                    (csrf(&s.jeton_csrf))
                                    input type="hidden" name="archive" value=(if cl.archive { "0" } else { "1" });
                                    button class={ "bouton bouton--large " @if cl.archive { "" } @else { "bouton--grave" } } {
                                        @if cl.archive { (t!("action.desarchiver")) } @else { (t!("action.archiver")) }
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
        @if peut && !cl.archive {
            a class="bouton bouton--vif" href=(format!("/operations/nouvelle?prereglage=livraison&client={id}")) {
                (t!("client.livrer_futs"))
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, cl.societe.clone())
        .sous_titre(format!("{} {}", cl.contact_prenom, cl.contact_nom))
        .actions(actions)
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

pub async fn modifier(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
    Form(f): Form<FormClient>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let securise = etat.config.cookies_securises;
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    persistance::client::modifier(
        tx.conn(),
        id,
        &persistance::client::DonneesClient {
            societe: &f.societe,
            contact_nom: &f.contact_nom,
            contact_prenom: &f.contact_prenom,
            telephone: &f.telephone,
            email: &f.email,
            commentaire: &f.commentaire,
        },
    )
    .await?;
    tx.valider().await?;
    Ok(rediriger(
        &format!("/clients/{id}"),
        "ok",
        t!("client.enregistre"),
        securise,
    ))
}

#[derive(Deserialize)]
pub struct FormArchive {
    archive: String,
}

/// Un client détenteur de fûts ne peut pas être archivé : la vérification
/// est faite côté serveur, pas seulement par le masquage du bouton.
pub async fn archiver(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
    Form(f): Form<FormArchive>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let securise = etat.config.cookies_securises;
    let archive = f.archive == "1";
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let cl = persistance::client::lire(tx.conn(), id).await?;
    if archive && cl.futs_detenus > 0 {
        return Ok(rediriger(
            &format!("/clients/{id}"),
            "grave",
            &t!("client.archive_impossible", n = cl.futs_detenus),
            securise,
        ));
    }
    persistance::client::archiver(tx.conn(), id, archive).await?;
    persistance::audit::tracer(
        tx.conn(),
        Some(s.utilisateur_id),
        if archive {
            "client.archive"
        } else {
            "client.reactive"
        },
        Some("client"),
        Some(&id.to_string()),
        serde_json::json!({}),
        None,
    )
    .await?;
    tx.valider().await?;
    Ok(rediriger(
        &format!("/clients/{id}"),
        "ok",
        t!("client.archive_ok"),
        securise,
    ))
}

#[derive(Deserialize)]
pub struct FormAdresse {
    #[serde(default)]
    libelle: String,
    #[serde(default)]
    rue: String,
    #[serde(default)]
    code_postal: String,
    #[serde(default)]
    ville: String,
    #[serde(default)]
    archive: Option<String>,
}

pub async fn ajouter_adresse(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
    Form(f): Form<FormAdresse>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let securise = etat.config.cookies_securises;
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    persistance::client::creer_adresse(
        tx.conn(),
        id,
        &persistance::client::DonneesAdresse {
            libelle: &f.libelle,
            rue: &f.rue,
            code_postal: &f.code_postal,
            ville: &f.ville,
        },
    )
    .await?;
    tx.valider().await?;
    Ok(rediriger(
        &format!("/clients/{id}"),
        "ok",
        t!("client.enregistre"),
        securise,
    ))
}

pub async fn modifier_adresse(
    State(etat): State<Etat>,
    Session(s): Session,
    Path((id, adresse_id)): Path<(Uuid, Uuid)>,
    Form(f): Form<FormAdresse>,
) -> Result<Response, ErreurWeb> {
    if !s.role.peut_ecrire() {
        return Err(ErreurWeb::Interdit);
    }
    let securise = etat.config.cookies_securises;
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    // Cohérence vérifiée côté serveur : une adresse d'un autre client est
    // refusée même si la requête est forgée à la main.
    if !persistance::client::adresse_appartient(tx.conn(), id, adresse_id).await? {
        return Err(ErreurWeb::Metier(
            domaine::ErreurMetier::AdresseEtrangereAuClient,
        ));
    }
    let archive = f
        .archive
        .as_deref()
        .map(|v| v == "1")
        .unwrap_or_else(|| false);
    persistance::client::modifier_adresse(
        tx.conn(),
        adresse_id,
        &persistance::client::DonneesAdresse {
            libelle: &f.libelle,
            rue: &f.rue,
            code_postal: &f.code_postal,
            ville: &f.ville,
        },
        archive,
    )
    .await?;
    tx.valider().await?;
    Ok(rediriger(
        &format!("/clients/{id}"),
        "ok",
        t!("client.enregistre"),
        securise,
    ))
}
