use axum::extract::State;
use axum::response::Response;
use domaine::{Role, Statut};
use maud::{html, Markup};

use crate::gabarits::composants as c;
use crate::gabarits::socle::{page, Cadre};
use crate::i18n::traduire;
use crate::middleware::Session;
use crate::routes::{page_avec_avis, Contexte};
use crate::{t, ErreurWeb, Etat};

/// Le tableau de bord n'est pas le même écran selon qui regarde.
///
/// Le bureau pilote un parc : compteurs, alertes, activité. L'atelier a une
/// main occupée et des gants mouillés : un bouton de scan, rien d'autre.
/// Ce n'est pas une largeur d'écran, c'est un rôle — le serveur sait qui
/// demande, aucune détection en JavaScript n'est nécessaire.
pub async fn afficher(
    State(etat): State<Etat>,
    ctx: Contexte,
    Session(s): Session,
) -> Result<Response, ErreurWeb> {
    let avait = ctx.avis.is_some();
    let mut tx = etat
        .depot
        .ouvrir(s.tenant_id, Some(s.utilisateur_id))
        .await?;

    let locataire = persistance::locataire::lire(tx.conn(), s.tenant_id).await?;
    let compteurs = persistance::fut::compter_par_statut(tx.conn()).await?;
    let nombre = |st: Statut| {
        compteurs
            .iter()
            .find(|x| x.statut == st)
            .map(|x| x.nombre)
            .unwrap_or(0)
    };

    let corps = if s.role == Role::Production {
        atelier(&s, nombre(Statut::Propre), nombre(Statut::Sale))
    } else {
        let non_revenus =
            persistance::fut::non_revenus(tx.conn(), locataire.seuil_alerte_non_revenu_jours, 8)
                .await?;
        let total_non_revenus = persistance::fut::non_revenus(
            tx.conn(),
            locataire.seuil_alerte_non_revenu_jours,
            10_000,
        )
        .await?
        .len() as i64;
        let dluo =
            persistance::lot::alertes_dluo(tx.conn(), locataire.seuil_alerte_dluo_jours).await?;
        let activite = persistance::evenement::activite_recente(tx.conn(), 12).await?;
        let rotation = persistance::fut::statistiques_rotation(tx.conn(), 180).await?;
        bureau(
            &s,
            &locataire,
            &compteurs,
            nombre(Statut::Propre),
            nombre(Statut::Livre),
            nombre(Statut::Sale),
            total_non_revenus,
            &non_revenus,
            &dluo,
            &activite,
            &rotation,
        )
    };
    tx.valider().await?;

    let cadre = Cadre::new(&s, &ctx.chemin, t!("tdb.titre"))
        .sous_titre(locataire.nom.clone())
        .avis(ctx.avis);
    Ok(page_avec_avis(
        page(cadre, corps),
        avait,
        etat.config.cookies_securises,
    ))
}

/// Écran d'atelier : une cible de 56 px minimum, un contraste élevé, et
/// tout le reste disparaît.
fn atelier(s: &persistance::session::ContexteSession, propres: i64, sales: i64) -> Markup {
    html! {
        div style="display:flex;flex-direction:column;gap:14px;max-width:560px" {
            a href="/scan" class="bouton bouton--vif bouton--atelier bouton--large"
              style="min-height:88px;font-size:21px" {
                (t!("nav.scan"))
            }
            a href="/operations/nouvelle" class="bouton bouton--atelier bouton--large" {
                (t!("nav.operation"))
            }
            p class="discret petit" { (t!("tdb.atelier_consigne")) }
            div class="cartes" {
                div class="tuile tuile--ok" {
                    span class="tuile__valeur num" { (propres) }
                    span class="tuile__nom" { (t!("tdb.propres_disponibles")) }
                }
                div class="tuile" {
                    span class="tuile__valeur num" { (sales) }
                    span class="tuile__nom" { (t!("tdb.a_laver")) }
                }
            }
            @let _ = s;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn bureau(
    s: &persistance::session::ContexteSession,
    locataire: &persistance::locataire::Locataire,
    compteurs: &[persistance::fut::CompteurStatut],
    propres: i64,
    chez_clients: i64,
    sales: i64,
    total_non_revenus: i64,
    non_revenus: &[persistance::fut::FutImmobilise],
    dluo: &[persistance::lot::AlerteDluo],
    activite: &[persistance::evenement::ActiviteRecente],
    rotation: &persistance::fut::StatsRotation,
) -> Markup {
    let total: i64 = compteurs.iter().map(|x| x.nombre).sum();
    html! {
        // Compteurs cliquables : chacun mène à la liste filtrée.
        div class="cartes" {
            a class="tuile tuile--ok" href="/parc?statut=propre" {
                span class="tuile__valeur num" { (propres) }
                span class="tuile__nom" { (t!("tdb.propres_disponibles")) }
            }
            a class="tuile" href="/parc?statut=livre" {
                span class="tuile__valeur num" { (chez_clients) }
                span class="tuile__nom" { (t!("tdb.chez_les_clients")) }
            }
            a class="tuile" href="/parc?statut=sale" {
                span class="tuile__valeur num" { (sales) }
                span class="tuile__nom" { (t!("tdb.a_laver")) }
            }
            a class={ "tuile " @if total_non_revenus > 0 { "tuile--grave" } @else { "" } }
              href="/parc?non_revenus=1" {
                span class="tuile__valeur num" { (total_non_revenus) }
                span class="tuile__nom" { (t!("tdb.non_revenus")) }
                span class="tuile__detail" {
                    (t!("tdb.non_revenus_detail", jours = locataire.seuil_alerte_non_revenu_jours))
                }
            }
        }

        div class="colonnes" style="margin-top:12px" {
            section class="carte" {
                div class="carte__tete" {
                    h2 { (t!("tdb.repartition")) }
                    span class="petit discret" { (total) " " "fûts" }
                }
                div class="carte__corps carte__corps--serre" {
                    div class="tableau-cadre" style="border:0" {
                        table class="tableau" {
                            tbody {
                                @for st in Statut::TOUS {
                                    @let n = compteurs.iter().find(|x| x.statut == st)
                                                 .map(|x| x.nombre).unwrap_or(0);
                                    tr {
                                        td { a href=(format!("/parc?statut={}", st.code()))
                                               style="text-decoration:none" {
                                            (c::badge_statut(st)) } }
                                        td class="num serre" style="font-weight:600" { (n) }
                                        td {
                                            @let pct = if total > 0 { (n as f64 / total as f64 * 100.0) as i64 } else { 0 };
                                            div style="height:6px;background:var(--surface-3);border-radius:3px;overflow:hidden" {
                                                div style=(format!("height:100%;width:{}%;background:var(--st-{})", pct, st.code())) {}
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
                div class="carte__tete" {
                    h2 { (t!("tdb.non_revenus")) }
                    a class="bouton bouton--petit" href="/parc?non_revenus=1" { (t!("action.voir")) }
                }
                div class="carte__corps carte__corps--serre" {
                    @if non_revenus.is_empty() {
                        div class="vide" { (t!("tdb.rien_a_signaler")) }
                    } @else {
                        div class="tableau-cadre" style="border:0" {
                            table class="tableau" {
                                tbody {
                                    @for f in non_revenus {
                                        tr {
                                            td { a class="lien-ligne" href=(format!("/futs/{}", f.id)) { (f.code_qr) } }
                                            td { (c::texte_ou_tiret(f.client.as_ref())) }
                                            td class="serre num" {
                                                (c::badge("grave", &format!("{} {}", f.jours, t!("tdb.jours"))))
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
                div class="carte__tete" {
                    h2 { (t!("tdb.alertes_dluo")) }
                    span class="petit discret" { (t!("tdb.alertes_dluo_detail")) }
                }
                div class="carte__corps carte__corps--serre" {
                    @if dluo.is_empty() {
                        div class="vide" { (t!("tdb.rien_a_signaler")) }
                    } @else {
                        div class="tableau-cadre" style="border:0" {
                            table class="tableau" {
                                tbody {
                                    @for a in dluo {
                                        tr {
                                            td { a class="lien-ligne" href=(format!("/parc?lot={}", a.lot_id)) { (a.numero) } }
                                            td class="petit" { (a.biere) }
                                            td class="serre" {
                                                @if a.jours_restants < 0 {
                                                    (c::badge("grave", t!("lot.dluo_depassee")))
                                                } @else {
                                                    (c::badge("alerte", &t!("lot.dluo_proche", n = a.jours_restants)))
                                                }
                                            }
                                            td class="serre num petit discret" { (a.futs_concernes) " fûts" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            section class="carte" {
                div class="carte__tete" { h2 { (t!("tdb.activite")) } }
                div class="carte__corps carte__corps--serre" {
                    @if activite.is_empty() {
                        div class="vide" { (t!("tdb.rien_a_signaler")) }
                    } @else {
                        div class="tableau-cadre" style="border:0" {
                            table class="tableau" {
                                tbody {
                                    @for e in activite {
                                        tr {
                                            td { a class="lien-ligne" href=(format!("/futs/{}", e.fut_id)) { (e.code_qr) } }
                                            td {
                                                @if let Some(st) = e.statut_apres { (c::badge_statut(st)) }
                                                @else { span class="petit discret" { (traduire(e.type_evenement.cle_i18n())) } }
                                            }
                                            td class="petit discret" { (c::texte_ou_tiret(e.par.as_ref())) }
                                            td class="serre petit discret num" { (c::depuis(e.survenu_le)) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        @if let Some(d) = rotation.duree_moyenne_chez_client_jours {
            div class="cartes" style="margin-top:12px" {
                div class="tuile" {
                    span class="tuile__valeur num" { (format!("{:.0}", d)) }
                    span class="tuile__nom" { (t!("tdb.duree_chez_client")) }
                    span class="tuile__detail" { (t!("tdb.jours")) " — " (rotation.rotations) " rotations" }
                }
            }
        }
        @let _ = s;
    }
}
