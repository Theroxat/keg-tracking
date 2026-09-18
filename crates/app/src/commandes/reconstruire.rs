use clap::Args as ClapArgs;

use super::{config, connecter};

#[derive(ClapArgs)]
pub struct Args {
    /// Identifiant court d'une brasserie. Omis, toutes sont traitées.
    #[arg(long)]
    pub locataire: Option<String>,
    /// Ne rien écrire : comparer seulement, et sortir en erreur s'il existe
    /// un écart. C'est ce que fait l'intégration continue.
    #[arg(long)]
    pub verifier: bool,
}

pub async fn executer(a: Args) -> anyhow::Result<()> {
    let config = config()?;
    let (depot, _) = connecter(&config).await?;

    let mut tx_ed = depot.ouvrir_editeur().await?;
    let locataires = persistance::locataire::lister(tx_ed.conn()).await?;
    tx_ed.valider().await?;

    let cibles: Vec<_> = locataires
        .into_iter()
        .filter(|l| {
            a.locataire
                .as_deref()
                .map(|s| l.identifiant_court == s)
                .unwrap_or(true)
        })
        .collect();

    if cibles.is_empty() {
        anyhow::bail!("aucune brasserie ne correspond");
    }

    let mut divergences_totales = 0usize;
    for l in &cibles {
        let mut tx = depot.ouvrir(l.id, None).await?;
        let rapport = persistance::reconstruction::executer(tx.conn(), !a.verifier).await?;
        if a.verifier {
            tx.annuler().await?;
        } else {
            tx.valider().await?;
        }

        println!(
            "{} : {} fûts examinés, {} divergences{}",
            l.identifiant_court,
            rapport.futs_examines,
            rapport.futs_diverges.len(),
            if a.verifier { "" } else { ", corrigées" }
        );
        for d in rapport.futs_diverges.iter().take(20) {
            println!("  {} :", d.code_qr);
            for detail in &d.details {
                println!("      {detail}");
            }
        }
        if rapport.futs_diverges.len() > 20 {
            println!("  … et {} autres", rapport.futs_diverges.len() - 20);
        }
        divergences_totales += rapport.futs_diverges.len();
    }

    depot.fermer().await;
    if a.verifier && divergences_totales > 0 {
        anyhow::bail!(
            "{divergences_totales} divergences entre le flux d'événements et les projections"
        );
    }
    println!("\nLe rejeu des événements redonne exactement l'état courant.");
    Ok(())
}
