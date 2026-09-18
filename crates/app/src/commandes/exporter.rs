//! Sortie de données et suppression d'un locataire.
//!
//! Un client doit pouvoir récupérer l'intégralité de ses données dans un
//! format ouvert, et une brasserie doit pouvoir être supprimée proprement.
//! Prévu dès le modèle, comme demandé.

use std::io::Write;

use clap::Args as ClapArgs;
use serde_json::json;

use super::{config, connecter};

#[derive(ClapArgs)]
pub struct Args {
    #[arg(long)]
    pub locataire: String,
    /// Fichier JSON de sortie. « - » écrit sur la sortie standard.
    #[arg(long, default_value = "export.json")]
    pub sortie: String,
}

/// Tables exportées, dans un ordre qui permet de relire l'archive.
const TABLES: &[&str] = &[
    "locataire",
    "utilisateur",
    "type_biere",
    "emplacement",
    "cuve",
    "client",
    "adresse_client",
    "lot",
    "fut",
    "evenement_fut",
    "document_mouvement",
    "ligne_document",
    "journal_audit",
];

pub async fn executer(a: Args) -> anyhow::Result<()> {
    let config = config()?;
    let (depot, _) = connecter(&config).await?;
    let locataire = trouver(&depot, &a.locataire).await?;

    let mut tx = depot.ouvrir(locataire.id, None).await?;
    let mut document = json!({
        "format": "webqr/export-locataire",
        "version": 1,
        "exporte_le": chrono::Utc::now().to_rfc3339(),
        "brasserie": locataire.identifiant_court,
        "tables": {},
    });

    for table in TABLES {
        // `row_to_json` évite d'énumérer les colonnes : l'export suit le
        // schéma sans maintenance. La RLS filtre déjà sur le locataire.
        let lignes: Vec<serde_json::Value> = sqlx::query_scalar(&format!(
            "select coalesce(json_agg(row_to_json(t)), '[]'::json) from {table} t"
        ))
        .fetch_one(tx.conn())
        .await?;
        document["tables"][table] = serde_json::Value::Array(lignes);
    }
    tx.valider().await?;

    let rendu = serde_json::to_string_pretty(&document)?;
    if a.sortie == "-" {
        println!("{rendu}");
    } else {
        let mut f = std::fs::File::create(&a.sortie)?;
        f.write_all(rendu.as_bytes())?;
        println!(
            "Export écrit dans {} ({} Kio)",
            a.sortie,
            rendu.len() / 1024
        );
    }
    depot.fermer().await;
    Ok(())
}

#[derive(ClapArgs)]
pub struct ArgsSuppression {
    #[arg(long)]
    pub locataire: String,
    /// Obligatoire : la suppression est définitive.
    #[arg(long)]
    pub confirmer: bool,
}

pub async fn supprimer(a: ArgsSuppression) -> anyhow::Result<()> {
    let config = config()?;
    let (depot, _) = connecter(&config).await?;
    let locataire = trouver(&depot, &a.locataire).await?;

    if !a.confirmer {
        anyhow::bail!(
            "suppression définitive de « {} » : relancez avec --confirmer. \
             Exportez d'abord ses données avec `webqr exporter-locataire`.",
            locataire.nom
        );
    }

    let mut tx = depot.ouvrir_editeur().await?;
    persistance::super_admin::tracer(
        tx.conn(),
        None,
        "brasserie.supprimee",
        None,
        json!({ "slug": locataire.identifiant_court, "nom": locataire.nom }),
        None,
    )
    .await?;
    // Les cascades du schéma emportent tout ce qui appartient au locataire.
    sqlx::query("delete from locataire where id = $1")
        .bind(locataire.id)
        .execute(tx.conn())
        .await?;
    tx.valider().await?;
    println!("Brasserie « {} » supprimée.", locataire.nom);
    depot.fermer().await;
    Ok(())
}

async fn trouver(
    depot: &persistance::Depot,
    slug: &str,
) -> anyhow::Result<persistance::locataire::Locataire> {
    let mut tx = depot.ouvrir_editeur().await?;
    let l = persistance::locataire::lister(tx.conn())
        .await?
        .into_iter()
        .find(|l| l.identifiant_court == slug);
    tx.valider().await?;
    l.ok_or_else(|| anyhow::anyhow!("brasserie « {slug} » introuvable"))
}
