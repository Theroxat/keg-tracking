//! Import de reprise du parc existant.
//!
//! Toutes les lignes sont validées avant la moindre écriture : un fichier
//! comportant une seule erreur n'écrit rien, et le rapport nomme chaque
//! ligne fautive. L'historique passé n'existe pas — il démarre à la reprise,
//! et l'écran le dit clairement.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use axum::extract::{Multipart, State};
use axum::response::Response;
use calamine::{Data, Reader, Xlsx};
use domaine::Statut;
use maud::html;
use uuid::Uuid;

use crate::gabarits::composants as c;
use crate::gabarits::csrf;
use crate::gabarits::socle::{page, Cadre};
use crate::middleware::Session;
use crate::routes::{page_avec_avis, Contexte};
use crate::{t, ErreurWeb, Etat};

const COLONNES_ATTENDUES: [&str; 2] = ["code_qr", "contenance"];

pub async fn afficher(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis_present();
    let cadre = Cadre::new(&s, &ctx.chemin, t!("import.titre")).avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, formulaire(&s, None)),
        avait,
        etat.config.cookies_securises,
    ))
}

/// Modèle de fichier : mêmes en-têtes que ceux que l'import attend.
pub async fn modele() -> impl axum::response::IntoResponse {
    let corps =
        "\u{feff}code_qr;contenance;statut;client;emplacement\r\nFUT-00001;30;propre;;Entrepôt\r\n";
    (
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"modele-import.csv\"",
            ),
        ],
        corps,
    )
}

fn formulaire(
    s: &persistance::session::ContexteSession,
    rapport: Option<&Rapport>,
) -> maud::Markup {
    html! {
        (c::retour_administration())
        div class="avis" { (t!("import.explication")) }
        p { a class="bouton bouton--petit" href="/administration/import/modele" { (t!("import.modele")) } }
        form method="post" action="/administration/import" enctype="multipart/form-data"
             style="max-width:620px" {
            (csrf(&s.jeton_csrf))
            div class="champ" {
                label for="champ-fichier" { (t!("import.fichier")) }
                input id="champ-fichier" type="file" name="fichier"
                      accept=".csv,.tsv,.xlsx,.xls" required;
            }
            (c::case_a_cocher("analyser", t!("import.analyser"), true))
            button class="bouton bouton--vif" { (t!("import.importer")) }
        }
        @if let Some(r) = rapport { (rendu_rapport(r)) }
    }
}

#[derive(Default)]
pub struct Rapport {
    lignes_lues: usize,
    erreurs: Vec<(usize, String)>,
    ecrit: usize,
    analyse_seule: bool,
}

fn rendu_rapport(r: &Rapport) -> maud::Markup {
    html! {
        section class="carte" style="margin-top:16px" {
            div class="carte__tete" {
                h2 { (t!("import.lignes_lues", n = r.lignes_lues)) }
                @if r.erreurs.is_empty() {
                    (c::badge("ok", &t!("import.reussi", n = r.ecrit)))
                } @else {
                    (c::badge("grave", &t!("import.erreurs", n = r.erreurs.len())))
                }
            }
            div class="carte__corps" {
                @if !r.erreurs.is_empty() {
                    div class="avis avis--grave" { (t!("import.rien_ecrit")) }
                    div class="tableau-cadre" {
                        table class="tableau" {
                            thead { tr { th { (t!("import.ligne", n = "")) } th { (t!("import.probleme")) } } }
                            tbody {
                                @for (n, msg) in r.erreurs.iter().take(200) {
                                    tr { td class="num serre" { (n) } td { (msg) } }
                                }
                            }
                        }
                    }
                    @if r.erreurs.len() > 200 {
                        p class="petit discret" { (t!("import.autres_erreurs", n = r.erreurs.len() - 200)) }
                    }
                } @else if r.analyse_seule {
                    div class="avis avis--ok" {
                        (t!("import.analyse_ok", analyser = t!("import.analyser"), n = r.lignes_lues))
                    }
                } @else {
                    div class="avis avis--ok" { (t!("import.reussi", n = r.ecrit)) }
                }
            }
        }
    }
}

struct LigneImport {
    numero: usize,
    code_qr: String,
    contenance: i32,
    statut: Statut,
    client: Option<String>,
    emplacement: Option<String>,
}

pub async fn traiter(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    mut formulaire_multipart: Multipart,
) -> Result<Response, ErreurWeb> {
    let mut octets: Vec<u8> = vec![];
    let mut nom_fichier = String::new();
    let mut analyse_seule = false;
    while let Some(champ) = formulaire_multipart
        .next_field()
        .await
        .map_err(|e| ErreurWeb::Interne(e.to_string()))?
    {
        match champ.name().unwrap_or("") {
            "fichier" => {
                nom_fichier = champ.file_name().unwrap_or("").to_lowercase();
                octets = champ
                    .bytes()
                    .await
                    .map_err(|e| ErreurWeb::Interne(e.to_string()))?
                    .to_vec();
            }
            "analyser" => analyse_seule = true,
            _ => {
                let _ = champ.bytes().await;
            }
        }
    }

    let brut = if nom_fichier.ends_with(".xlsx") || nom_fichier.ends_with(".xls") {
        lire_tableur(&octets)
    } else {
        lire_csv(&octets)
    };

    let mut rapport = Rapport {
        analyse_seule,
        ..Default::default()
    };
    let lignes_brutes = match brut {
        Ok(l) => l,
        Err(e) => {
            rapport.erreurs.push((0, e));
            return rendre(&etat, ctx, &s, rapport).await;
        }
    };
    rapport.lignes_lues = lignes_brutes.len();

    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    let clients = persistance::client::lister(tx.conn(), true, None).await?;
    let emplacements = persistance::referentiel::lister_emplacements(tx.conn(), false).await?;
    let index_clients: HashMap<String, Uuid> = clients
        .iter()
        .map(|c| (c.societe.to_lowercase(), c.id))
        .collect();
    let index_emplacements: HashMap<String, Uuid> = emplacements
        .iter()
        .map(|e| (e.nom.to_lowercase(), e.id))
        .collect();

    // ---- validation complète avant la moindre écriture
    let mut valides: Vec<LigneImport> = vec![];
    let mut vus: HashSet<String> = HashSet::new();
    for (i, cellules) in lignes_brutes.iter().enumerate() {
        let n = i + 2; // en-tête en ligne 1
        let code = cellules
            .get("code_qr")
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_uppercase();
        if code.is_empty() {
            rapport
                .erreurs
                .push((n, t!("import.colonne_manquante", colonne = "code_qr")));
            continue;
        }
        if !vus.insert(code.clone()) {
            rapport
                .erreurs
                .push((n, format!("{code} : {}", t!("erreur.deja_utilise"))));
            continue;
        }
        let contenance = cellules
            .get("contenance")
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(0);
        if !loc.contenances_autorisees.contains(&contenance) {
            rapport.erreurs.push((
                n,
                crate::i18n::message_erreur(&domaine::ErreurMetier::ContenanceNonAutorisee {
                    litres: contenance,
                }),
            ));
            continue;
        }
        let statut = cellules
            .get("statut")
            .map(|v| normaliser(v))
            .and_then(|v| Statut::depuis_code(&v))
            .unwrap_or(Statut::Recu);

        let client = cellules
            .get("client")
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        if let Some(cl) = &client {
            if !index_clients.contains_key(&cl.to_lowercase()) {
                rapport
                    .erreurs
                    .push((n, format!("{} : « {cl} »", t!("erreur.introuvable"))));
                continue;
            }
            if statut != Statut::Livre {
                rapport.erreurs.push((
                    n,
                    format!(
                        "un client n'a de sens que pour un fût {}",
                        t!("statut.livre")
                    ),
                ));
                continue;
            }
        } else if statut == Statut::Livre {
            rapport.erreurs.push((
                n,
                crate::i18n::message_erreur(&domaine::ErreurMetier::PreconditionManquante {
                    statut,
                    champ: "client",
                }),
            ));
            continue;
        }

        let emplacement = cellules
            .get("emplacement")
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        if let Some(e) = &emplacement {
            if !index_emplacements.contains_key(&e.to_lowercase()) {
                rapport
                    .erreurs
                    .push((n, format!("{} : « {e} »", t!("erreur.introuvable"))));
                continue;
            }
        }

        valides.push(LigneImport {
            numero: n,
            code_qr: code,
            contenance,
            statut,
            client,
            emplacement,
        });
    }

    // Codes déjà présents dans le parc.
    let tous: Vec<String> = valides.iter().map(|l| l.code_qr.clone()).collect();
    for pris in persistance::fut::codes_deja_pris(tx.conn(), &tous).await? {
        if let Some(l) = valides.iter().find(|l| l.code_qr == pris) {
            rapport
                .erreurs
                .push((l.numero, format!("{pris} : {}", t!("erreur.deja_utilise"))));
        }
    }

    if !rapport.erreurs.is_empty() || analyse_seule {
        rapport.erreurs.sort_by_key(|(n, _)| *n);
        tx.annuler().await?;
        return rendre(&etat, ctx, &s, rapport).await;
    }

    // ---- écriture, en une seule transaction
    for l in &valides {
        let adresse = match &l.client {
            Some(cl) => {
                let cid = index_clients[&cl.to_lowercase()];
                persistance::client::adresse_unique(tx.conn(), cid)
                    .await?
                    .map(|a| (cid, a))
            }
            None => None,
        };
        let emplacement_id = l
            .emplacement
            .as_ref()
            .and_then(|e| index_emplacements.get(&e.to_lowercase()).copied())
            .or(loc.emplacement_defaut_id);

        let id = persistance::fut::creer(
            tx.conn(),
            &persistance::fut::NouveauFut {
                code_qr: &l.code_qr,
                contenance_litres: l.contenance,
                date_mise_en_service: chrono::Utc::now().date_naive(),
                commentaire: "",
                statut: Statut::Recu,
                emplacement_id,
            },
            Some(s.utilisateur_id),
        )
        .await?;

        // L'état repris est posé par un événement d'import explicite : la
        // chronologie commence donc par « ce que l'on savait à la reprise ».
        if l.statut != Statut::Recu || adresse.is_some() {
            let mut d =
                persistance::evenement::DemandeEvenement::nouvelle(id, domaine::Origine::Import);
            d.statut_cible = Some(l.statut);
            d.type_impose = Some(domaine::TypeEvenement::Import);
            d.commentaire = "Reprise du parc existant".into();
            d.ctx.emplacement_id = emplacement_id;
            if let Some((cid, aid)) = adresse {
                d.ctx.client_id = Some(cid);
                d.ctx.adresse_client_id = Some(aid);
            }
            let verrou = persistance::evenement::verrouiller(tx.conn(), id).await?;
            let utilisateur = Some(s.utilisateur_id);
            persistance::evenement::ecrire(
                tx.conn(),
                utilisateur,
                Some(verrou.statut),
                verrou.etat,
                &d,
            )
            .await?;
        }
        rapport.ecrit += 1;
    }

    persistance::audit::tracer(
        tx.conn(),
        Some(s.utilisateur_id),
        "import.reprise",
        Some("fut"),
        None,
        serde_json::json!({ "lignes": rapport.lignes_lues, "ecrit": rapport.ecrit }),
        None,
    )
    .await?;
    tx.valider().await?;
    rendre(&etat, ctx, &s, rapport).await
}

async fn rendre(
    etat: &Etat,
    ctx: Contexte,
    s: &persistance::session::ContexteSession,
    rapport: Rapport,
) -> Result<Response, ErreurWeb> {
    let cadre = Cadre::new(s, &ctx.chemin, t!("import.titre"));
    Ok(page_avec_avis(
        page(cadre, formulaire(s, Some(&rapport))),
        false,
        etat.config.cookies_securises,
    ))
}

fn normaliser(v: &str) -> String {
    v.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'û' | 'ù' | 'ü' => 'u',
            'ô' | 'ö' => 'o',
            'î' | 'ï' => 'i',
            'ç' => 'c',
            ' ' | '-' => '_',
            autre => autre,
        })
        .collect()
}

type Lignes = Vec<HashMap<String, String>>;

fn verifier_entetes(entetes: &[String]) -> Result<(), String> {
    for attendue in COLONNES_ATTENDUES {
        if !entetes.iter().any(|e| e == attendue) {
            return Err(t!("import.colonne_manquante", colonne = attendue));
        }
    }
    Ok(())
}

fn lire_csv(octets: &[u8]) -> Result<Lignes, String> {
    let texte = String::from_utf8_lossy(octets);
    let texte = texte.trim_start_matches('\u{feff}');
    let separateur = if texte
        .lines()
        .next()
        .map(|l| l.contains(';'))
        .unwrap_or(false)
    {
        b';'
    } else {
        b','
    };
    let mut lecteur = csv::ReaderBuilder::new()
        .delimiter(separateur)
        .flexible(true)
        .from_reader(texte.as_bytes());
    let entetes: Vec<String> = lecteur
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(normaliser)
        .collect();
    verifier_entetes(&entetes)?;

    let mut lignes = vec![];
    for enreg in lecteur.records() {
        let enreg = enreg.map_err(|e| e.to_string())?;
        if enreg.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        lignes.push(
            entetes
                .iter()
                .cloned()
                .zip(enreg.iter().map(|c| c.to_string()))
                .collect(),
        );
    }
    Ok(lignes)
}

fn lire_tableur(octets: &[u8]) -> Result<Lignes, String> {
    let mut classeur: Xlsx<_> =
        Xlsx::new(Cursor::new(octets.to_vec())).map_err(|e| e.to_string())?;
    let noms = classeur.sheet_names().to_vec();
    let premiere = noms.first().ok_or("classeur vide")?;
    let plage = classeur
        .worksheet_range(premiere)
        .map_err(|e| e.to_string())?;

    let mut iter = plage.rows();
    let entetes: Vec<String> = iter
        .next()
        .ok_or("feuille vide")?
        .iter()
        .map(|c| normaliser(&cellule(c)))
        .collect();
    verifier_entetes(&entetes)?;

    let mut lignes = vec![];
    for ligne in iter {
        if ligne.iter().all(|c| cellule(c).trim().is_empty()) {
            continue;
        }
        lignes.push(
            entetes
                .iter()
                .cloned()
                .zip(ligne.iter().map(cellule))
                .collect(),
        );
    }
    Ok(lignes)
}

fn cellule(d: &Data) -> String {
    match d {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        autre => autre.to_string(),
    }
}
