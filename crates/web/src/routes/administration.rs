use axum::extract::{Path, State};
use axum::response::Response;
use axum::Form;
use domaine::Role;
use maud::{html, Markup};
use serde::Deserialize;
use uuid::Uuid;

use crate::gabarits::composants as c;
use crate::gabarits::csrf;
use crate::gabarits::socle::{page, Cadre};
use crate::i18n::traduire;
use crate::middleware::Session;
use crate::routes::{page_avec_avis, rediriger, Contexte};
use crate::{t, ErreurWeb, Etat};

// ------------------------------------------------------------------ accueil
pub async fn accueil(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let tuile = |href: &str, titre: &str, detail: &str| {
        html! {
            a class="tuile" href=(href) {
                span class="tuile__nom" style="font-size:15px;font-weight:620" { (titre) }
                span class="tuile__detail" { (detail) }
            }
        }
    };
    let corps = html! {
        div class="cartes" {
            (tuile("/administration/referentiels", t!("ref.titre"), t!("ref.explication")))
            (tuile("/administration/utilisateurs", t!("util.titre"), t!("role.administrateur.description")))
            (tuile("/lots", t!("lot.titre"), t!("rappel.titre")))
            (tuile("/administration/etiquettes", t!("etiq.titre"), t!("etiq.planche_note")))
            (tuile("/administration/import", t!("import.titre"), t!("import.explication")))
            (tuile("/administration/parametres", t!("param.titre"), t!("param.seuil_non_revenu")))
            (tuile("/administration/journal", t!("journal.titre"), t!("journal.action")))
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("admin.titre")).avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

// ------------------------------------------------------------ référentiels
pub async fn referentiels(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let bieres = persistance::referentiel::lister_bieres(tx.conn(), false).await?;
    let emplacements = persistance::referentiel::lister_emplacements(tx.conn(), false).await?;
    let cuves = persistance::referentiel::lister_cuves(tx.conn(), false).await?;
    tx.valider().await?;

    let jeton = s.jeton_csrf.clone();
    let simple = |titre: &str, genre: &str, entrees: &[persistance::referentiel::Entree]| {
        html! {
            section class="carte" {
                div class="carte__tete" { h2 { (titre) } span class="petit discret" { (entrees.len()) } }
                div class="carte__corps" {
                    @for e in entrees {
                        form method="post" action=(format!("/administration/referentiels/{genre}/{}", e.id))
                             style="display:flex;gap:6px;align-items:center;margin-bottom:8px" {
                            (csrf(&jeton))
                            input type="text" name="nom" value=(e.nom) required;
                            label class="coche" style="white-space:nowrap" {
                                input type="checkbox" name="actif" value="1" checked[e.actif];
                                span class="petit" { (t!("champ.actif")) }
                            }
                            span class="petit discret" style="white-space:nowrap" { (t!("ref.en_usage", n = e.futs)) }
                            button class="bouton bouton--petit" { (t!("action.enregistrer")) }
                        }
                    }
                    form method="post" action=(format!("/administration/referentiels/{genre}"))
                         style="display:flex;gap:6px;margin-top:12px;padding-top:12px;border-top:1px solid var(--bordure)" {
                        (csrf(&jeton))
                        input type="text" name="nom" placeholder=(t!("champ.nom")) required;
                        button class="bouton bouton--vif bouton--petit" { (t!("action.ajouter")) }
                    }
                }
            }
        }
    };

    let corps = html! {
        div class="avis" { (t!("ref.explication")) }
        div style="display:grid;gap:12px;grid-template-columns:repeat(auto-fit,minmax(320px,1fr))" {
            section class="carte" {
                div class="carte__tete" { h2 { (t!("ref.bieres")) } span class="petit discret" { (bieres.len()) } }
                div class="carte__corps" {
                    @for b in &bieres {
                        form method="post" action=(format!("/administration/referentiels/biere/{}", b.id))
                             style="display:flex;gap:6px;align-items:center;margin-bottom:8px" {
                            (csrf(&jeton))
                            input type="color" name="couleur" value=(b.couleur)
                                  style="width:40px;min-height:34px;padding:2px";
                            input type="text" name="nom" value=(b.nom) required;
                            input type="number" name="ordre" value=(b.ordre) style="width:64px"
                                  aria-label=(t!("champ.ordre"));
                            label class="coche" { input type="checkbox" name="actif" value="1" checked[b.actif];
                                                  span class="petit" { (t!("champ.actif")) } }
                            span class="petit discret" style="white-space:nowrap" { (t!("ref.en_usage", n = b.futs)) }
                            button class="bouton bouton--petit" { (t!("action.enregistrer")) }
                        }
                    }
                    form method="post" action="/administration/referentiels/biere"
                         style="display:flex;gap:6px;margin-top:12px;padding-top:12px;border-top:1px solid var(--bordure)" {
                        (csrf(&jeton))
                        input type="color" name="couleur" value="#c08a3e" style="width:40px;min-height:34px;padding:2px";
                        input type="text" name="nom" placeholder=(t!("champ.nom")) required;
                        button class="bouton bouton--vif bouton--petit" { (t!("action.ajouter")) }
                    }
                }
            }
            (simple(t!("ref.emplacements"), "emplacement", &emplacements))
            (simple(t!("ref.cuves"), "cuve", &cuves))
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("ref.titre")).avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct FormReferentiel {
    nom: String,
    #[serde(default)]
    couleur: Option<String>,
    #[serde(default)]
    ordre: Option<i32>,
    #[serde(default)]
    actif: Option<String>,
}

pub async fn creer_referentiel(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(genre): Path<String>,
    Form(f): Form<FormReferentiel>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let resultat = match genre.as_str() {
        "biere" => persistance::referentiel::creer_biere(
            tx.conn(),
            &f.nom,
            f.couleur.as_deref().unwrap_or("#c08a3e"),
            f.ordre.unwrap_or(0),
        )
        .await
        .map(|_| ()),
        "emplacement" => persistance::referentiel::creer_emplacement(tx.conn(), &f.nom)
            .await
            .map(|_| ()),
        "cuve" => persistance::referentiel::creer_cuve(tx.conn(), &f.nom)
            .await
            .map(|_| ()),
        _ => return Err(ErreurWeb::Introuvable),
    };
    match resultat {
        Ok(()) => {
            tx.valider().await?;
            Ok(rediriger(
                "/administration/referentiels",
                "ok",
                t!("ref.enregistre"),
                securise,
            ))
        }
        Err(persistance::Erreur::Conflit(_)) => Ok(rediriger(
            "/administration/referentiels",
            "grave",
            t!("ref.nom_deja_pris"),
            securise,
        )),
        Err(e) => Err(e.into()),
    }
}

pub async fn modifier_referentiel(
    State(etat): State<Etat>,
    Session(s): Session,
    Path((genre, id)): Path<(String, Uuid)>,
    Form(f): Form<FormReferentiel>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let actif = f.actif.is_some();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let resultat = match genre.as_str() {
        "biere" => {
            persistance::referentiel::modifier_biere(
                tx.conn(),
                id,
                &f.nom,
                f.couleur.as_deref().unwrap_or("#c08a3e"),
                f.ordre.unwrap_or(0),
                actif,
            )
            .await
        }
        "emplacement" => {
            persistance::referentiel::modifier_emplacement(tx.conn(), id, &f.nom, actif).await
        }
        "cuve" => persistance::referentiel::modifier_cuve(tx.conn(), id, &f.nom, actif).await,
        _ => return Err(ErreurWeb::Introuvable),
    };
    match resultat {
        Ok(()) => {
            tx.valider().await?;
            Ok(rediriger(
                "/administration/referentiels",
                "ok",
                t!("ref.enregistre"),
                securise,
            ))
        }
        Err(persistance::Erreur::Conflit(_)) => Ok(rediriger(
            "/administration/referentiels",
            "grave",
            t!("ref.nom_deja_pris"),
            securise,
        )),
        Err(e) => Err(e.into()),
    }
}

// -------------------------------------------------------------- utilisateurs
pub async fn utilisateurs(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let liste = persistance::utilisateur::lister(tx.conn()).await?;
    tx.valider().await?;
    let tz = s.fuseau_horaire.clone();

    let corps = html! {
        div class="tableau-cadre" {
            table class="tableau" {
                thead { tr {
                    th { (t!("champ.nom")) } th { (t!("champ.email")) } th { (t!("champ.role")) }
                    th { (t!("champ.pin")) } th { "Dernière connexion" } th { (t!("champ.actif")) } th {}
                } }
                tbody {
                    @for u in &liste {
                        tr {
                            td style="font-weight:600" {
                                (format!("{} {}", u.prenom, u.nom).trim())
                                @if u.id == s.utilisateur_id { " " (c::badge("neutre", "vous")) }
                            }
                            td class="petit" { (u.email) }
                            td { (c::badge("neutre", traduire(u.role.cle_i18n()))) }
                            td { @if u.a_un_pin { (c::badge("ok", "✓")) } @else { span class="discret" { "—" } } }
                            td class="petit discret num" {
                                @match u.derniere_connexion_le {
                                    Some(d) => (c::date_heure(d, &tz)),
                                    None => (t!("app.jamais")),
                                }
                            }
                            td { @if u.actif { (c::badge("ok", t!("app.oui"))) } @else { (c::badge("grave", t!("app.non"))) } }
                            td class="serre" {
                                a class="bouton bouton--petit" href=(format!("/administration/utilisateurs/{}", u.id)) {
                                    (t!("action.modifier"))
                                }
                            }
                        }
                    }
                }
            }
        }
        section class="carte" style="margin-top:14px;max-width:640px" {
            div class="carte__tete" { h2 { (t!("util.nouveau")) } }
            div class="carte__corps" {
                form method="post" action="/administration/utilisateurs" {
                    (csrf(&s.jeton_csrf))
                    div class="grille" {
                        (c::Champ::texte("prenom", t!("champ.prenom"), "").rendre())
                        (c::Champ::texte("nom", t!("champ.nom"), "").requis().rendre())
                    }
                    (c::Champ::texte("email", t!("champ.email"), "").de_type("email").requis().rendre())
                    (c::selection("role", t!("champ.role"),
                        &Role::TOUS.iter().map(|r| (r.code().to_string(),
                            format!("{} — {}", traduire(r.cle_i18n()),
                                    traduire(&format!("{}.description", r.cle_i18n())))))
                            .collect::<Vec<_>>(),
                        Some("lecture"), None, true))
                    p class="champ__aide" { (t!("util.mot_de_passe_temporaire_note")) }
                    button class="bouton bouton--vif" { (t!("action.creer")) }
                }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("util.titre")).avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct FormUtilisateur {
    #[serde(default)]
    prenom: String,
    nom: String,
    #[serde(default)]
    email: String,
    role: String,
    #[serde(default)]
    actif: Option<String>,
    #[serde(default)]
    reinitialiser: Option<String>,
}

pub async fn creer_utilisateur(
    State(etat): State<Etat>,
    Session(s): Session,
    Form(f): Form<FormUtilisateur>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let role = Role::depuis_code(&f.role).ok_or(ErreurWeb::Introuvable)?;
    let mot_de_passe = mot_de_passe_temporaire();
    let hachage = etat
        .hacheur
        .hacher(&mot_de_passe)
        .map_err(|e| ErreurWeb::Interne(e.to_string()))?;

    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let resultat = persistance::utilisateur::creer(
        tx.conn(),
        &persistance::utilisateur::NouvelUtilisateur {
            tenant_id: s.tenant_id,
            email: &f.email.trim().to_lowercase(),
            mot_de_passe_hash: &hachage,
            nom: &f.nom,
            prenom: &f.prenom,
            role,
        },
    )
    .await;
    match resultat {
        Ok(id) => {
            persistance::audit::tracer(
                tx.conn(),
                Some(s.utilisateur_id),
                "utilisateur.cree",
                Some("utilisateur"),
                Some(&id.to_string()),
                serde_json::json!({ "email": f.email, "role": role.code() }),
                None,
            )
            .await?;
            tx.valider().await?;
            Ok(rediriger(
                "/administration/utilisateurs",
                "ok",
                &format!(
                    "{} — {} : {}",
                    t!("util.enregistre"),
                    t!("util.mot_de_passe_temporaire"),
                    mot_de_passe
                ),
                securise,
            ))
        }
        Err(persistance::Erreur::Conflit(_)) => Ok(rediriger(
            "/administration/utilisateurs",
            "grave",
            t!("util.email_deja_pris"),
            securise,
        )),
        Err(e) => Err(e.into()),
    }
}

pub async fn fiche_utilisateur(
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
    let u = persistance::utilisateur::lire(tx.conn(), id).await?;
    tx.valider().await?;
    let soi_meme = u.id == s.utilisateur_id;

    let corps = html! {
        form method="post" action=(format!("/administration/utilisateurs/{id}")) style="max-width:600px" {
            (csrf(&s.jeton_csrf))
            @if soi_meme {
                div class="avis avis--alerte" { (t!("util.pas_soi_meme")) }
            }
            div class="grille" {
                (c::Champ::texte("prenom", t!("champ.prenom"), &u.prenom).rendre())
                (c::Champ::texte("nom", t!("champ.nom"), &u.nom).requis().rendre())
            }
            (c::Champ::texte("email_affiche", t!("champ.email"), &u.email).rendre())
            (c::selection("role", t!("champ.role"),
                &Role::TOUS.iter().map(|r| (r.code().to_string(), traduire(r.cle_i18n()).to_string()))
                    .collect::<Vec<_>>(), Some(u.role.code()), None, true))
            (c::case_a_cocher("actif", t!("champ.actif"), u.actif))
            div class="boutons" {
                button class="bouton bouton--vif" { (t!("action.enregistrer")) }
                button class="bouton bouton--grave" name="reinitialiser" value="1" {
                    (t!("util.reinitialiser_mdp"))
                }
                a class="bouton bouton--nu" href="/administration/utilisateurs" { (t!("app.retour")) }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, format!("{} {}", u.prenom, u.nom))
        .sous_titre(u.email.clone())
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

pub async fn modifier_utilisateur(
    State(etat): State<Etat>,
    Session(s): Session,
    Path(id): Path<Uuid>,
    Form(f): Form<FormUtilisateur>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let role = Role::depuis_code(&f.role).ok_or(ErreurWeb::Introuvable)?;
    let actif = f.actif.is_some();
    let retour = "/administration/utilisateurs";

    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let u = persistance::utilisateur::lire(tx.conn(), id).await?;

    // Un administrateur ne peut ni se rétrograder ni se désactiver.
    if id == s.utilisateur_id && (role != u.role || !actif) {
        return Ok(rediriger(
            retour,
            "grave",
            t!("util.pas_soi_meme"),
            securise,
        ));
    }
    // Une brasserie doit toujours conserver au moins un administrateur actif.
    if u.role == Role::Administrateur && u.actif && (role != Role::Administrateur || !actif) {
        let restants = persistance::utilisateur::compter_administrateurs_actifs(tx.conn()).await?;
        if restants <= 1 {
            return Ok(rediriger(
                retour,
                "grave",
                t!("util.dernier_admin"),
                securise,
            ));
        }
    }

    if f.reinitialiser.is_some() {
        let nouveau = mot_de_passe_temporaire();
        let hachage = etat
            .hacheur
            .hacher(&nouveau)
            .map_err(|e| ErreurWeb::Interne(e.to_string()))?;
        persistance::utilisateur::changer_mot_de_passe(tx.conn(), id, &hachage).await?;
        persistance::utilisateur::definir_pin(tx.conn(), id, None).await?;
        persistance::audit::tracer(
            tx.conn(),
            Some(s.utilisateur_id),
            "utilisateur.mot_de_passe_reinitialise",
            Some("utilisateur"),
            Some(&id.to_string()),
            serde_json::json!({}),
            None,
        )
        .await?;
        tx.valider().await?;
        let _ = persistance::session::revoquer_utilisateur(etat.depot.auth(), id).await;
        return Ok(rediriger(
            retour,
            "ok",
            &format!("{} : {}", t!("util.mot_de_passe_temporaire"), nouveau),
            securise,
        ));
    }

    persistance::utilisateur::modifier(tx.conn(), id, &f.nom, &f.prenom, role, actif).await?;
    persistance::audit::tracer(
        tx.conn(),
        Some(s.utilisateur_id),
        "utilisateur.modifie",
        Some("utilisateur"),
        Some(&id.to_string()),
        serde_json::json!({ "role": role.code(), "actif": actif }),
        None,
    )
    .await?;
    tx.valider().await?;
    if !actif || role != u.role {
        let _ = persistance::session::revoquer_utilisateur(etat.depot.auth(), id).await;
    }
    Ok(rediriger(retour, "ok", t!("util.enregistre"), securise))
}

fn mot_de_passe_temporaire() -> String {
    use rand::Rng;
    const MOTS: &[&str] = &[
        "houblon", "levure", "brassin", "malte", "cuve", "garde", "trempe", "filtre", "ambree",
        "blonde", "saison", "tirage",
    ];
    let mut rng = rand::thread_rng();
    let m: Vec<&str> = (0..3).map(|_| MOTS[rng.gen_range(0..MOTS.len())]).collect();
    format!("{}-{}!A", m.join("-"), rng.gen_range(100..999))
}

// --------------------------------------------------------------- paramètres
pub async fn parametres(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    let emplacements = persistance::referentiel::lister_emplacements(tx.conn(), true).await?;
    tx.valider().await?;

    let corps = html! {
        form method="post" action="/administration/parametres" style="max-width:640px" {
            (csrf(&s.jeton_csrf))
            (c::Champ::texte("nom", t!("champ.nom"), &loc.nom).requis().rendre())
            (c::Champ::texte("prefixe_qr", t!("etiq.prefixe"), &loc.prefixe_qr)
                .requis().aide("Majuscules et chiffres. Unique sur toute l'installation.").rendre())
            (c::selection("emplacement_defaut_id", t!("param.emplacement_defaut"),
                &emplacements.iter().map(|e| (e.id.to_string(), e.nom.clone())).collect::<Vec<_>>(),
                loc.emplacement_defaut_id.map(|i| i.to_string()).as_deref(), Some("—"), false))
            div class="grille" {
                (c::Champ::texte("seuil_alerte_non_revenu_jours", t!("param.seuil_non_revenu"),
                    loc.seuil_alerte_non_revenu_jours.to_string()).de_type("number").requis().rendre())
                (c::Champ::texte("seuil_alerte_dluo_jours", t!("param.seuil_dluo"),
                    loc.seuil_alerte_dluo_jours.to_string()).de_type("number").requis().rendre())
            }
            (c::Champ::texte("contenances", t!("param.contenances"),
                loc.contenances_autorisees.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))
                .aide(t!("param.contenances_aide")).rendre())
            (c::Champ::texte("fuseau_horaire", t!("param.fuseau"), &loc.fuseau_horaire).requis().rendre())
            button class="bouton bouton--vif" { (t!("action.enregistrer")) }
        }
        section class="carte" style="margin-top:16px;max-width:640px" {
            div class="carte__tete" { h2 { (t!("param.export_donnees")) } }
            div class="carte__corps" {
                p class="petit discret" { (t!("param.export_donnees_aide")) }
                p class="petit mono" { "webqr exporter-locataire --locataire " (loc.identifiant_court) }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("param.titre")).avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct FormParametres {
    nom: String,
    prefixe_qr: String,
    #[serde(default)]
    emplacement_defaut_id: Option<String>,
    seuil_alerte_non_revenu_jours: i32,
    seuil_alerte_dluo_jours: i32,
    contenances: String,
    fuseau_horaire: String,
}

pub async fn enregistrer_parametres(
    State(etat): State<Etat>,
    Session(s): Session,
    Form(f): Form<FormParametres>,
) -> Result<Response, ErreurWeb> {
    let securise = etat.config.cookies_securises;
    let contenances: Vec<i32> = f
        .contenances
        .split(',')
        .filter_map(|x| x.trim().parse::<i32>().ok())
        .filter(|x| *x > 0)
        .collect();
    if contenances.is_empty() {
        return Ok(rediriger(
            "/administration/parametres",
            "grave",
            t!("erreur.nombre_invalide"),
            securise,
        ));
    }
    if f.fuseau_horaire.parse::<chrono_tz::Tz>().is_err() {
        return Ok(rediriger(
            "/administration/parametres",
            "grave",
            t!("param.fuseau"),
            securise,
        ));
    }

    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    persistance::locataire::enregistrer_parametres(
        tx.conn(),
        s.tenant_id,
        &persistance::locataire::ParametresLocataire {
            nom: f.nom.trim().to_string(),
            prefixe_qr: f.prefixe_qr.trim().to_uppercase(),
            emplacement_defaut_id: f
                .emplacement_defaut_id
                .as_deref()
                .filter(|x| !x.is_empty())
                .and_then(|x| Uuid::parse_str(x).ok()),
            fuseau_horaire: f.fuseau_horaire.trim().to_string(),
            seuil_alerte_non_revenu_jours: f.seuil_alerte_non_revenu_jours.max(1),
            seuil_alerte_dluo_jours: f.seuil_alerte_dluo_jours.max(0),
            contenances_autorisees: contenances,
        },
    )
    .await?;
    persistance::audit::tracer(
        tx.conn(),
        Some(s.utilisateur_id),
        "parametres.modifies",
        Some("locataire"),
        Some(&s.tenant_id.to_string()),
        serde_json::json!({}),
        None,
    )
    .await?;
    tx.valider().await?;
    Ok(rediriger(
        "/administration/parametres",
        "ok",
        t!("param.enregistre"),
        securise,
    ))
}

// ------------------------------------------------------------------ journal
pub async fn journal(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let entrees = persistance::audit::lister(tx.conn(), 300).await?;
    tx.valider().await?;
    let tz = s.fuseau_horaire.clone();

    let corps = html! {
        @if entrees.is_empty() {
            (c::vide(t!("journal.aucune_entree"), None))
        } @else {
            div class="tableau-cadre" {
                table class="tableau" {
                    thead { tr {
                        th { (t!("journal.quand")) } th { (t!("journal.qui")) }
                        th { (t!("journal.action")) } th { (t!("journal.cible")) } th { "Détails" }
                    } }
                    tbody {
                        @for e in &entrees {
                            tr {
                                td class="petit num discret" { (c::date_heure(e.survenu_le, &tz)) }
                                td class="petit" { (c::texte_ou_tiret(e.utilisateur.as_ref())) }
                                td { (c::badge("neutre", &e.action)) }
                                td class="petit discret mono" {
                                    (e.cible_type.clone().unwrap_or_default())
                                }
                                td class="petit discret" {
                                    @if e.details.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                                        (e.details.to_string())
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    let cadre = Cadre::new(&s, &ctx.chemin, t!("journal.titre")).avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

pub fn _m() -> Markup {
    html! {}
}
