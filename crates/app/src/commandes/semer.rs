//! Jeu de démonstration.
//!
//! Indispensable pour développer, pour tester la tenue en charge, et pour
//! montrer le produit à un prospect sans inventer des chiffres à la volée.
//! L'historique généré est crédible : des cycles complets étalés sur un an,
//! des fûts immobilisés chez des clients, des lots dont la DLUO approche.

use chrono::{Duration, NaiveDate, Utc};
use clap::Args as ClapArgs;
use domaine::Statut;
use rand::seq::SliceRandom;
use rand::Rng;
use uuid::Uuid;

use super::{config, connecter};

#[derive(ClapArgs)]
pub struct Args {
    /// Identifiant court de la brasserie à peupler.
    #[arg(long)]
    pub locataire: String,
    #[arg(long, default_value_t = 3000)]
    pub futs: usize,
    #[arg(long, default_value_t = 18)]
    pub clients: usize,
    #[arg(long, default_value_t = 24)]
    pub lots: usize,
}

const BIERES: &[(&str, &str)] = &[
    ("Blonde de saison", "#d9a441"),
    ("Ambrée d'automne", "#b06a2c"),
    ("Triple abbaye", "#c98a2e"),
    ("IPA houblonnée", "#8ea63a"),
    ("Stout impérial", "#3b2a22"),
    ("Blanche de blé", "#e2cf95"),
];
const EMPLACEMENTS: &[&str] = &["Brasserie", "Stock 1", "Stock 2", "Zone de lavage", "Quai"];
const CUVES: &[&str] = &[
    "Cuve 1",
    "Cuve 2",
    "Cuve 3",
    "Cuve 4",
    "Fermenteur A",
    "Fermenteur B",
];
const SOCIETES: &[&str] = &[
    "Le Comptoir des Halles",
    "Brasserie du Port",
    "Caves Saint-Michel",
    "Bar L'Ancre",
    "Restaurant La Tonnelle",
    "Épicerie Fine Duval",
    "Le Bistrot des Amis",
    "Chez Marcel",
    "La Cave à Bulles",
    "Taverne du Vieux Moulin",
    "Hôtel des Voyageurs",
    "Le Zinc",
    "Comptoir Général",
    "Brasserie Lafayette",
    "La Buvette du Lac",
    "Le Tire-Bouchon",
    "Maison Renaud",
    "Bar à Bières Houblon",
];
const VILLES: &[(&str, &str)] = &[
    ("Nantes", "44000"),
    ("Rennes", "35000"),
    ("Angers", "49000"),
    ("Vannes", "56000"),
    ("Saint-Nazaire", "44600"),
    ("Laval", "53000"),
    ("Cholet", "49300"),
    ("Lorient", "56100"),
];
const PRENOMS: &[&str] = &[
    "Camille", "Lucas", "Sophie", "Thomas", "Émilie", "Nicolas", "Julie", "Marc",
];
const NOMS: &[&str] = &[
    "Berger", "Moreau", "Girard", "Leroy", "Perrin", "Dubois", "Marchand", "Colin",
];

pub async fn executer(a: Args) -> anyhow::Result<()> {
    let config = config()?;
    let (depot, _) = connecter(&config).await?;

    let mut tx_ed = depot.ouvrir_editeur().await?;
    let locataire = persistance::locataire::lister(tx_ed.conn())
        .await?
        .into_iter()
        .find(|l| l.identifiant_court == a.locataire)
        .ok_or_else(|| anyhow::anyhow!("brasserie « {} » introuvable", a.locataire))?;
    tx_ed.valider().await?;

    let mut tx = depot.ouvrir(locataire.id, None).await?;
    let conn = tx.conn();
    let mut rng = rand::thread_rng();

    // ------------------------------------------------------ référentiels
    // Insertions idempotentes : la commande doit pouvoir être rejouée, et
    // le provisionnement a déjà créé quelques emplacements. Dans une
    // transaction PostgreSQL, un échec de contrainte avorte tout le bloc —
    // on évite donc l'erreur plutôt que de la rattraper.
    for (i, (nom, couleur)) in BIERES.iter().enumerate() {
        sqlx::query(
            "insert into type_biere (tenant_id, nom, couleur, ordre)
             values (locataire_courant(), $1, $2, $3) on conflict do nothing",
        )
        .bind(nom)
        .bind(couleur)
        .bind(i as i32)
        .execute(&mut *conn)
        .await?;
    }
    let bieres: Vec<Uuid> = sqlx::query_scalar("select id from type_biere order by ordre, nom")
        .fetch_all(&mut *conn)
        .await?;

    sqlx::query(
        "insert into emplacement (tenant_id, nom)
         select locataire_courant(), n from unnest($1::text[]) as n on conflict do nothing",
    )
    .bind(
        EMPLACEMENTS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    )
    .execute(&mut *conn)
    .await?;
    let emplacements: Vec<Uuid> = sqlx::query_scalar("select id from emplacement order by nom")
        .fetch_all(&mut *conn)
        .await?;

    sqlx::query(
        "insert into cuve (tenant_id, nom)
         select locataire_courant(), n from unnest($1::text[]) as n on conflict do nothing",
    )
    .bind(CUVES.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    .execute(&mut *conn)
    .await?;
    let cuves: Vec<Uuid> = sqlx::query_scalar("select id from cuve order by nom")
        .fetch_all(&mut *conn)
        .await?;

    // ------------------------------------------------------ clients
    let clients_existants: i64 = sqlx::query_scalar("select count(*) from client")
        .fetch_one(&mut *conn)
        .await?;
    let mut adresses: Vec<(Uuid, Uuid)> = vec![];
    for i in 0..if clients_existants > 0 {
        0
    } else {
        a.clients.min(SOCIETES.len())
    } {
        let (ville, cp) = VILLES[i % VILLES.len()];
        let client_id = persistance::client::creer(
            &mut *conn,
            &persistance::client::DonneesClient {
                societe: SOCIETES[i],
                contact_nom: NOMS[i % NOMS.len()],
                contact_prenom: PRENOMS[i % PRENOMS.len()],
                telephone: &format!(
                    "02 {:02} {:02} {:02} {:02}",
                    rng.gen_range(10..99),
                    rng.gen_range(10..99),
                    rng.gen_range(10..99),
                    rng.gen_range(10..99)
                ),
                email: &format!("contact{}@exemple.fr", i + 1),
                commentaire: "",
            },
        )
        .await?;
        let n_adresses = if i % 5 == 0 { 2 } else { 1 };
        for j in 0..n_adresses {
            let adresse_id = persistance::client::creer_adresse(
                &mut *conn,
                client_id,
                &persistance::client::DonneesAdresse {
                    libelle: if j == 0 { "Principale" } else { "Annexe" },
                    rue: &format!("{} rue des Brasseurs", rng.gen_range(1..90)),
                    code_postal: cp,
                    ville,
                },
            )
            .await?;
            adresses.push((client_id, adresse_id));
        }
    }

    // ------------------------------------------------------ lots
    let aujourdhui = Utc::now().date_naive();
    if clients_existants > 0 {
        adresses = sqlx::query_as("select client_id, id from adresse_client where not archive")
            .fetch_all(&mut *conn)
            .await?;
    }

    let lots_existants: i64 = sqlx::query_scalar("select count(*) from lot")
        .fetch_one(&mut *conn)
        .await?;
    let mut lots: Vec<(Uuid, Uuid, Uuid)> = vec![];
    for i in 0..if lots_existants > 0 { 0 } else { a.lots } {
        let biere = *bieres.choose(&mut rng).unwrap();
        let cuve = *cuves.choose(&mut rng).unwrap();
        let brassage = aujourdhui - chrono::Duration::days((a.lots - i) as i64 * 14);
        // Quelques lots dont la DLUO approche ou vient de passer : l'alerte
        // du tableau de bord doit avoir de quoi s'illustrer.
        let dluo = brassage + chrono::Duration::days(if i >= a.lots - 3 { 100 } else { 180 });
        let lot_id = persistance::lot::creer(
            &mut *conn,
            &persistance::lot::DonneesLot {
                numero: &format!("{}-{:03}", brassage.format("%Y"), i + 1),
                date_brassage: brassage,
                type_biere_id: biere,
                cuve_id: cuve,
                dluo: Some(dluo),
                commentaire: "",
            },
        )
        .await?;
        lots.push((lot_id, biere, cuve));
    }

    if lots_existants > 0 {
        lots = sqlx::query_as("select id, type_biere_id, cuve_id from lot")
            .fetch_all(&mut *conn)
            .await?;
    }
    anyhow::ensure!(
        !lots.is_empty() && !adresses.is_empty(),
        "référentiels incomplets"
    );

    // ------------------------------------------------------ fûts
    let codes: Vec<String> = (1..=a.futs)
        .map(|n| format!("{}{:05}", locataire.prefixe_qr, n))
        .collect();
    let deja = persistance::fut::codes_deja_pris(&mut *conn, &codes).await?;
    let a_creer: Vec<String> = codes.into_iter().filter(|c| !deja.contains(c)).collect();
    let contenances = [20i32, 30, 50];
    for tranche in a_creer.chunks(500) {
        let c = *contenances.choose(&mut rng).unwrap();
        persistance::fut::creer_en_masse(
            &mut *conn,
            tranche,
            c,
            emplacements.first().copied(),
            None,
        )
        .await?;
    }

    // La séquence de départ se lit sur le flux d'événements, pas sur la
    // projection : le flux est la vérité, la projection un cache.
    let futs: Vec<(Uuid, i64)> = sqlx::query_as(
        "select f.id, coalesce(max(e.sequence), 0)
           from fut f left join evenement_fut e on e.fut_id = f.id
          group by f.id order by f.id",
    )
    .fetch_all(&mut *conn)
    .await?;

    // ------------------------------------------------------ historique
    // Des cycles complets sur un an, générés puis insérés en masse. La
    // projection est ensuite recalculée par le rejeu : le jeu de démo
    // exerce donc aussi le chemin de reconstruction.
    let mut ids = vec![];
    let mut fut_ids = vec![];
    let mut seqs = vec![];
    let mut types = vec![];
    let mut origines = vec![];
    let mut avant = vec![];
    let mut apres = vec![];
    let mut empls: Vec<Option<Uuid>> = vec![];
    let mut clis: Vec<Option<Uuid>> = vec![];
    let mut adrs: Vec<Option<Uuid>> = vec![];
    let mut bies: Vec<Option<Uuid>> = vec![];
    let mut lts: Vec<Option<Uuid>> = vec![];
    let mut cvs: Vec<Option<Uuid>> = vec![];
    let mut quands = vec![];

    let maintenant = Utc::now();
    for (fut_id, seq_depart) in &futs {
        let mut sequence = *seq_depart;
        let mut statut = Statut::Recu;
        let mut instant = maintenant - Duration::days(rng.gen_range(150..365));
        // Une part du parc s'immobilise chez un client : c'est précisément le
        // problème que le produit doit rendre visible.
        let s_immobilise = rng.gen_bool(0.05);
        // Quelques fûts sortent du parc : casse, ou disparition constatée.
        let sortie = rng.gen_range(0..1000);

        // On avance dans le cycle jusqu'à rattraper le présent. Le fût
        // s'arrête donc là où le temps le laisse, ce qui répartit
        // naturellement le parc entre tous les statuts.
        'cycle: loop {
            for etape in [
                Statut::Sale,
                Statut::Propre,
                Statut::Rempli,
                Statut::Livre,
                Statut::Recu,
            ] {
                let duree = match etape {
                    Statut::Sale => Duration::hours(rng.gen_range(2..48)),
                    Statut::Propre => Duration::hours(rng.gen_range(6..96)),
                    Statut::Rempli => Duration::days(rng.gen_range(1..14)),
                    Statut::Livre => Duration::days(rng.gen_range(1..5)),
                    _ => Duration::days(rng.gen_range(4..40)),
                };
                if instant + duree >= maintenant {
                    break 'cycle;
                }
                instant += duree;
                sequence += 1;

                let (mut b, mut l, mut cv, mut cl, mut ad) = (None, None, None, None, None);
                match etape {
                    Statut::Rempli => {
                        let (lot, biere, cuve) = *lots.choose(&mut rng).unwrap();
                        b = Some(biere);
                        l = Some(lot);
                        cv = Some(cuve);
                    }
                    Statut::Livre => {
                        let (c, ad_id) = *adresses.choose(&mut rng).unwrap();
                        cl = Some(c);
                        ad = Some(ad_id);
                    }
                    _ => {}
                }

                ids.push(Uuid::now_v7());
                fut_ids.push(*fut_id);
                seqs.push(sequence);
                types.push("transition".to_string());
                origines.push(
                    if rng.gen_bool(0.7) {
                        "document"
                    } else {
                        "scan"
                    }
                    .to_string(),
                );
                avant.push(statut.code().to_string());
                apres.push(etape.code().to_string());
                empls.push(if etape == Statut::Livre {
                    None
                } else {
                    emplacements.choose(&mut rng).copied()
                });
                clis.push(cl);
                adrs.push(ad);
                bies.push(b);
                lts.push(l);
                cvs.push(cv);
                quands.push(instant);
                statut = etape;

                // Le fût part chez un client et n'en revient pas.
                if s_immobilise
                    && etape == Statut::Livre
                    && instant < maintenant - Duration::days(45)
                {
                    break 'cycle;
                }
            }
        }

        // 1 % hors service, 0,6 % perdus : un parc réel n'est pas intact.
        if sortie < 16 && statut != Statut::Livre {
            sequence += 1;
            ids.push(Uuid::now_v7());
            fut_ids.push(*fut_id);
            seqs.push(sequence);
            types.push("transition".to_string());
            origines.push("edition_manuelle".to_string());
            avant.push(statut.code().to_string());
            apres.push(
                if sortie < 10 {
                    Statut::HorsService
                } else {
                    Statut::Perdu
                }
                .code()
                .to_string(),
            );
            empls.push(None);
            clis.push(None);
            adrs.push(None);
            bies.push(None);
            lts.push(None);
            cvs.push(None);
            quands.push(maintenant - Duration::days(rng.gen_range(1..90)));
        }
    }

    // `cle_i18n` renvoie « evenement.transition » : on remet le code SQL.
    for t in types.iter_mut() {
        if t.starts_with("evenement.") {
            *t = t.trim_start_matches("evenement.").to_string();
        }
    }

    let total = ids.len();
    for debut in (0..total).step_by(5000) {
        let fin = (debut + 5000).min(total);
        let plage = debut..fin;
        sqlx::query(
            r#"insert into evenement_fut
                 (id, tenant_id, fut_id, sequence, type_evenement, origine,
                  statut_avant, statut_apres, emplacement_id, client_id,
                  adresse_client_id, type_biere_id, lot_id, cuve_id, survenu_le,
                  justification, commentaire)
               select u.id, locataire_courant(), u.fut_id, u.seq,
                      u.te::type_evenement, u.og::origine_evenement,
                      u.sa::statut_fut, u.sp::statut_fut,
                      u.empl, u.cli, u.adr, u.bie, u.lot, u.cuv, u.quand,
                      case when u.te = 'transition_forcee'
                           then 'Reprise de donnees' else '' end,
                      ''
                 from unnest($1::uuid[], $2::uuid[], $3::bigint[], $4::text[], $5::text[],
                             $6::text[], $7::text[], $8::uuid[], $9::uuid[], $10::uuid[],
                             $11::uuid[], $12::uuid[], $13::uuid[], $14::timestamptz[])
                   as u(id, fut_id, seq, te, og, sa, sp, empl, cli, adr, bie, lot, cuv, quand)"#,
        )
        .bind(&ids[plage.clone()])
        .bind(&fut_ids[plage.clone()])
        .bind(&seqs[plage.clone()])
        .bind(&types[plage.clone()])
        .bind(&origines[plage.clone()])
        .bind(&avant[plage.clone()])
        .bind(&apres[plage.clone()])
        .bind(&empls[plage.clone()])
        .bind(&clis[plage.clone()])
        .bind(&adrs[plage.clone()])
        .bind(&bies[plage.clone()])
        .bind(&lts[plage.clone()])
        .bind(&cvs[plage.clone()])
        .bind(&quands[plage])
        .execute(&mut *conn)
        .await?;
    }

    // Les projections sont recalculées par rejeu : c'est exactement le
    // chemin que la commande de reconstruction emprunte en production.
    let rapport = persistance::reconstruction::executer(&mut *conn, true).await?;
    tx.valider().await?;

    println!("Jeu de démonstration injecté dans « {} » :", locataire.nom);
    println!("  {} fûts", futs.len());
    println!("  {} événements générés", total);
    println!(
        "  {} clients, {} lots, {} bières",
        adresses.len(),
        lots.len(),
        bieres.len()
    );
    println!(
        "  {} projections calculées par rejeu",
        rapport.futs_corriges
    );
    let _ = NaiveDate::default();
    depot.fermer().await;
    Ok(())
}
