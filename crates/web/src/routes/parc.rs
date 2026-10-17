use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use domaine::Statut;
use maud::{html, Markup};
use serde::Deserialize;
use uuid::Uuid;

use crate::gabarits::composants as c;
use crate::gabarits::socle::{page, Cadre};
use crate::i18n::traduire;
use crate::middleware::Session;
use crate::routes::{page_avec_avis, Contexte};
use crate::{t, ErreurWeb, Etat};

use persistance::fut::{Filtres, Tri};

const PAR_PAGE: i64 = 50;

#[derive(Deserialize, Default, Clone)]
pub struct Requete {
    pub q: Option<String>,
    /// Codes séparés par des virgules : `propre,rempli`.
    pub statut: Option<String>,
    pub client: Option<Uuid>,
    pub lot: Option<Uuid>,
    pub biere: Option<Uuid>,
    pub emplacement: Option<Uuid>,
    pub cuve: Option<Uuid>,
    pub contenance: Option<i32>,
    pub non_revenus: Option<u8>,
    pub dluo: Option<u8>,
    pub tri: Option<String>,
    pub sens: Option<String>,
    pub page: Option<i64>,
}

impl Requete {
    fn statuts(&self) -> Vec<Statut> {
        self.statut
            .as_deref()
            .map(|s| s.split(',').filter_map(Statut::depuis_code).collect())
            .unwrap_or_default()
    }

    fn filtres(&self, seuil_non_revenu: i32, seuil_dluo: i32) -> Filtres {
        Filtres {
            recherche: self
                .q
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            statuts: self.statuts(),
            client_id: self.client,
            lot_id: self.lot,
            type_biere_id: self.biere,
            emplacement_id: self.emplacement,
            cuve_id: self.cuve,
            contenance: self.contenance,
            non_revenu_jours: (self.non_revenus == Some(1)).then_some(seuil_non_revenu),
            dluo_dans_jours: (self.dluo == Some(1)).then_some(seuil_dluo),
        }
    }

    fn tri(&self) -> (Tri, bool) {
        (
            Tri::depuis_code(self.tri.as_deref().unwrap_or("code_qr")),
            self.sens.as_deref() == Some("desc"),
        )
    }

    /// Reconstruit l'URL en conservant les filtres : le tri, la pagination
    /// et l'export doivent porter exactement la même vue.
    fn base(&self, chemin: &str, sans: &[&str]) -> String {
        let mut p: Vec<String> = vec![];
        let mut pousser = |cle: &str, val: String| {
            if !sans.contains(&cle) && !val.is_empty() {
                p.push(format!("{cle}={}", urlencoding::encode(&val)));
            }
        };
        if let Some(v) = &self.q {
            pousser("q", v.clone());
        }
        if let Some(v) = &self.statut {
            pousser("statut", v.clone());
        }
        if let Some(v) = self.client {
            pousser("client", v.to_string());
        }
        if let Some(v) = self.lot {
            pousser("lot", v.to_string());
        }
        if let Some(v) = self.biere {
            pousser("biere", v.to_string());
        }
        if let Some(v) = self.emplacement {
            pousser("emplacement", v.to_string());
        }
        if let Some(v) = self.cuve {
            pousser("cuve", v.to_string());
        }
        if let Some(v) = self.contenance {
            pousser("contenance", v.to_string());
        }
        if self.non_revenus == Some(1) {
            pousser("non_revenus", "1".into());
        }
        if self.dluo == Some(1) {
            pousser("dluo", "1".into());
        }
        if let Some(v) = &self.tri {
            pousser("tri", v.clone());
        }
        if let Some(v) = &self.sens {
            pousser("sens", v.clone());
        }
        if let Some(v) = self.page {
            pousser("page", v.to_string());
        }
        if p.is_empty() {
            format!("{chemin}?")
        } else {
            format!("{chemin}?{}&", p.join("&"))
        }
    }
}

pub async fn afficher(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
    Query(r): Query<Requete>,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis.is_some();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    let filtres = r.filtres(
        loc.seuil_alerte_non_revenu_jours,
        loc.seuil_alerte_dluo_jours,
    );
    let (tri, desc) = r.tri();
    let page_courante = r.page.unwrap_or(1).max(1);

    let total = persistance::fut::compter(tx.conn(), &filtres).await?;
    let futs = persistance::fut::lister(
        tx.conn(),
        &filtres,
        tri,
        desc,
        page_courante,
        PAR_PAGE.min(persistance::fut::PAGE_MAX),
    )
    .await?;

    let bieres = persistance::referentiel::lister_bieres(tx.conn(), false).await?;
    let emplacements = persistance::referentiel::lister_emplacements(tx.conn(), false).await?;
    let clients = persistance::client::lister(tx.conn(), false, None).await?;
    tx.valider().await?;

    let total_pages = (total + PAR_PAGE - 1) / PAR_PAGE;
    let base_tri = r.base("/parc", &["tri", "sens", "page"]);
    let base_page = r.base("/parc", &["page"]);
    let base_export = r.base("/parc/export", &["page"]);
    let tri_code = tri.code();

    let corps = html! {
        form class="filtres" method="get" action="/parc" {
            div class="champ champ--large" {
                label for="champ-q" { (t!("action.rechercher")) }
                input id="champ-q" type="search" name="q" value=[r.q.as_deref()]
                      placeholder=(t!("parc.recherche_placeholder"));
            }
            // Sur téléphone, les filtres secondaires sont repliés : sinon
            // ils occupent tout le premier écran avant la moindre donnée.
            // Bascule en CSS pure, sans JavaScript, et sans `name` pour que
            // la case ne parte pas dans la requête.
            input type="checkbox" id="filtres-plus" class="bascule-filtres";
            div class="filtres__avances" {
            div class="champ" {
                label for="champ-statut" { (t!("champ.statut")) }
                select id="champ-statut" name="statut" {
                    option value="" { (t!("app.tous")) }
                    @for st in Statut::TOUS {
                        option value=(st.code())
                               selected[r.statut.as_deref() == Some(st.code())] {
                            (traduire(st.cle_i18n()))
                        }
                    }
                }
            }
            div class="champ" {
                label for="champ-biere" { (t!("champ.biere")) }
                select id="champ-biere" name="biere" {
                    option value="" { (t!("app.toutes")) }
                    @for b in &bieres {
                        option value=(b.id) selected[r.biere == Some(b.id)] { (b.nom) }
                    }
                }
            }
            div class="champ" {
                label for="champ-emplacement" { (t!("champ.emplacement")) }
                select id="champ-emplacement" name="emplacement" {
                    option value="" { (t!("app.tous")) }
                    @for e in &emplacements {
                        option value=(e.id) selected[r.emplacement == Some(e.id)] { (e.nom) }
                    }
                }
            }
            div class="champ" {
                label for="champ-client" { (t!("champ.client")) }
                select id="champ-client" name="client" {
                    option value="" { (t!("app.tous")) }
                    @for cl in &clients {
                        option value=(cl.id) selected[r.client == Some(cl.id)] { (cl.societe) }
                    }
                }
            }
            }
            div class="boutons" {
                label for="filtres-plus" class="bouton bascule-filtres__bouton" {
                    (t!("parc.plus_de_filtres"))
                }
                button type="submit" class="bouton bouton--vif" { (t!("action.filtrer")) }
                @if filtres.actifs() {
                    a class="bouton bouton--nu" href="/parc" { (t!("action.reinitialiser")) }
                }
            }
        }

        @if filtres.actifs() {
            div class="jetons" {
                @if r.non_revenus == Some(1) {
                    span class="jeton" {
                        (t!("tdb.non_revenus_detail", jours = loc.seuil_alerte_non_revenu_jours))
                        a href=(r.base("/parc", &["non_revenus", "page"]).trim_end_matches('&')) { "×" }
                    }
                }
                @if r.dluo == Some(1) {
                    span class="jeton" {
                        (t!("tdb.alertes_dluo"))
                        a href=(r.base("/parc", &["dluo", "page"]).trim_end_matches('&')) { "×" }
                    }
                }
                @if let Some(l) = r.lot {
                    span class="jeton" { (t!("champ.lot")) " " (l)
                        a href=(r.base("/parc", &["lot", "page"]).trim_end_matches('&')) { "×" } }
                }
            }
        }

        div style="display:flex;justify-content:space-between;align-items:center;gap:10px;margin-bottom:10px;flex-wrap:wrap" {
            p class="minus" style="margin:0" {
                @if total == 1 { (t!("parc.resultat")) } @else { (t!("parc.resultats", n = total)) }
            }
            div class="boutons sans-impression" {
                a class="bouton bouton--petit" href=(format!("{base_export}format=csv")) {
                    (t!("action.exporter_csv"))
                }
                a class="bouton bouton--petit" href=(format!("{base_export}format=xlsx")) {
                    (t!("action.exporter_excel"))
                }
            }
        }

        @if futs.is_empty() {
            (c::vide(
                if filtres.actifs() { t!("parc.aucun_fut") } else { t!("parc.parc_vide") },
                (!filtres.actifs() && s.role == domaine::Role::Administrateur).then(|| html! {
                    div class="boutons" {
                        a class="bouton bouton--vif" href="/administration/etiquettes" { (t!("etiq.titre")) }
                        a class="bouton" href="/administration/import" { (t!("import.titre")) }
                    }
                }),
            ))
        } @else {
            div class="tableau-cadre" {
                table class="tableau tableau--cartes" {
                    thead { tr {
                        (c::entete_tri(t!("champ.code_qr"), "code_qr", tri_code, desc, &base_tri))
                        (c::entete_tri(t!("champ.statut"), "statut", tri_code, desc, &base_tri))
                        th { (t!("champ.biere")) }
                        th { (t!("champ.lot")) }
                        th { (t!("champ.cuve")) }
                        (c::entete_tri(t!("champ.emplacement"), "emplacement", tri_code, desc, &base_tri))
                        (c::entete_tri(t!("champ.client"), "client", tri_code, desc, &base_tri))
                        (c::entete_tri(t!("champ.anciennete"), "anciennete", tri_code, desc, &base_tri))
                        (c::entete_tri(t!("champ.derniere_activite"), "activite", tri_code, desc, &base_tri))
                    } }
                    tbody {
                        @for f in &futs {
                            // `cel-titre` / `cel-marque` / `data-libelle` pilotent la
                            // bascule en cartes sous 720 px : sur téléphone, une table
                            // de neuf colonnes ne se lit pas.
                            tr {
                                td class="cel-titre" {
                                    a class="lien-ligne" href=(format!("/futs/{}", f.id)) { (f.code_qr) }
                                }
                                td class="cel-marque" { (c::badge_statut(f.statut)) }
                                td data-libelle=(t!("champ.biere")) {
                                    @if let Some(b) = &f.biere {
                                        span style="display:inline-flex;align-items:center;gap:6px" {
                                            (c::pastille_biere(f.couleur_biere.as_ref())) (b)
                                        }
                                    } @else { span class="discret" { "—" } }
                                }
                                td class="mono petit" data-libelle=(t!("champ.lot")) {
                                    (c::texte_ou_tiret(f.lot.as_ref()))
                                }
                                td class="petit" data-libelle=(t!("champ.cuve")) {
                                    (c::texte_ou_tiret(f.cuve.as_ref()))
                                }
                                td class="petit" data-libelle=(t!("champ.emplacement")) {
                                    (c::texte_ou_tiret(f.emplacement.as_ref()))
                                }
                                td data-libelle=(t!("champ.client")) {
                                    @if let (Some(cl), Some(id)) = (&f.client, f.client_id) {
                                        a href=(format!("/clients/{id}")) { (cl) }
                                    } @else { span class="discret" { "—" } }
                                }
                                td class="petit num" data-libelle=(t!("champ.anciennete")) {
                                    (c::depuis(f.statut_depuis_le))
                                }
                                td class="petit discret num" data-libelle=(t!("champ.derniere_activite")) {
                                    @match f.dernier_evenement_le {
                                        Some(d) => (c::depuis(d)),
                                        None => span class="discret" { "—" },
                                    }
                                }
                            }
                        }
                    }
                }
            }
            (c::pagination(page_courante, total_pages, &base_page))
        }
    };

    let cadre = Cadre::new(&s, &ctx.chemin, t!("parc.titre"))
        .sous_titre(if total == 1 {
            t!("parc.total_un").to_string()
        } else {
            t!("parc.total", n = total)
        })
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

#[derive(Deserialize)]
pub struct RequeteExport {
    #[serde(default)]
    pub format: String,
    #[serde(flatten)]
    pub filtres: Requete,
}

/// L'export reflète exactement les filtres actifs : c'est un besoin
/// quotidien du bureau, pas une commodité.
pub async fn exporter(
    State(etat): State<Etat>,
    Session(s): Session,
    Query(r): Query<RequeteExport>,
) -> Result<Response, ErreurWeb> {
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;
    let loc = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    let filtres = r.filtres.filtres(
        loc.seuil_alerte_non_revenu_jours,
        loc.seuil_alerte_dluo_jours,
    );
    let (tri, desc) = r.filtres.tri();
    let futs = persistance::fut::lister_tout(tx.conn(), &filtres, tri, desc).await?;
    tx.valider().await?;

    let entetes = [
        t!("champ.code_qr"),
        t!("champ.contenance"),
        t!("champ.statut"),
        t!("champ.depuis"),
        t!("champ.biere"),
        t!("champ.lot"),
        t!("champ.dluo"),
        t!("champ.cuve"),
        t!("champ.emplacement"),
        t!("champ.client"),
        t!("champ.date_remplissage"),
        t!("champ.derniere_activite"),
        t!("champ.date_mise_en_service"),
    ];
    let ligne = |f: &persistance::fut::Fut| -> Vec<String> {
        vec![
            f.code_qr.clone(),
            f.contenance_litres.to_string(),
            traduire(f.statut.cle_i18n()).to_string(),
            c::date_heure(f.statut_depuis_le, &s.fuseau_horaire),
            f.biere.clone().unwrap_or_default(),
            f.lot.clone().unwrap_or_default(),
            f.dluo.map(c::jour).unwrap_or_default(),
            f.cuve.clone().unwrap_or_default(),
            f.emplacement.clone().unwrap_or_default(),
            f.client.clone().unwrap_or_default(),
            f.date_remplissage
                .map(|d| c::date_heure(d, &s.fuseau_horaire))
                .unwrap_or_default(),
            f.dernier_evenement_le
                .map(|d| c::date_heure(d, &s.fuseau_horaire))
                .unwrap_or_default(),
            c::jour(f.date_mise_en_service),
        ]
    };
    let horodatage = chrono::Utc::now().format("%Y%m%d-%H%M");

    if r.format == "xlsx" {
        let mut classeur = rust_xlsxwriter::Workbook::new();
        let feuille = classeur.add_worksheet();
        feuille.set_name("Parc").ok();
        let gras = rust_xlsxwriter::Format::new().set_bold();
        for (i, e) in entetes.iter().enumerate() {
            feuille
                .write_string_with_format(0, i as u16, *e, &gras)
                .ok();
        }
        for (n, f) in futs.iter().enumerate() {
            for (i, v) in ligne(f).iter().enumerate() {
                feuille.write_string(n as u32 + 1, i as u16, v).ok();
            }
        }
        feuille.set_freeze_panes(1, 0).ok();
        let octets = classeur.save_to_buffer()?;
        return Ok((
            [
                (
                    header::CONTENT_TYPE,
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
                ),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"parc-{horodatage}.xlsx\""),
                ),
            ],
            octets,
        )
            .into_response());
    }

    let mut ecrivain = csv::WriterBuilder::new()
        .delimiter(b';')
        .from_writer(vec![]);
    ecrivain.write_record(entetes).ok();
    for f in &futs {
        ecrivain.write_record(ligne(f)).ok();
    }
    let mut corps = String::from("\u{feff}"); // BOM : Excel ouvre l'UTF-8 correctement
    corps.push_str(&String::from_utf8_lossy(
        &ecrivain
            .into_inner()
            .map_err(|e| ErreurWeb::Interne(e.to_string()))?,
    ));
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"parc-{horodatage}.csv\""),
            ),
        ],
        corps,
    )
        .into_response())
}

/// Utilisé par la fiche client et l'écran de rappel.
pub fn lien_parc_filtre(cle: &str, valeur: &Uuid) -> String {
    format!("/parc?{cle}={valeur}")
}

pub fn _inutilise() -> Markup {
    html! {}
}
