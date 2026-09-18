//! Point d'entrée : le serveur et les commandes d'exploitation.

mod commandes;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "webqr",
    about = "Traçabilité de fûts de bière par QR code",
    version
)]
struct Cli {
    #[command(subcommand)]
    commande: Commande,
}

#[derive(Subcommand)]
enum Commande {
    /// Démarre le serveur web.
    Serve,

    /// Crée une brasserie et son premier administrateur.
    ProvisionnerBrasserie(commandes::provisionner::Args),

    /// Crée un compte éditeur (back-office hors locataire).
    CreerEditeur(commandes::provisionner::ArgsEditeur),

    /// Injecte un jeu de démonstration complet et crédible.
    SemerDemonstration(commandes::semer::Args),

    /// Rejoue le flux d'événements et recalcule les projections.
    ReconstruireProjections(commandes::reconstruire::Args),

    /// Exporte l'intégralité des données d'une brasserie.
    ExporterLocataire(commandes::exporter::Args),

    /// Supprime définitivement une brasserie et toutes ses données.
    SupprimerLocataire(commandes::exporter::ArgsSuppression),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("RUST_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        match cli.commande {
            Commande::Serve => commandes::serve::executer().await,
            Commande::ProvisionnerBrasserie(a) => commandes::provisionner::brasserie(a).await,
            Commande::CreerEditeur(a) => commandes::provisionner::editeur(a).await,
            Commande::SemerDemonstration(a) => commandes::semer::executer(a).await,
            Commande::ReconstruireProjections(a) => commandes::reconstruire::executer(a).await,
            Commande::ExporterLocataire(a) => commandes::exporter::executer(a).await,
            Commande::SupprimerLocataire(a) => commandes::exporter::supprimer(a).await,
        }
    })
}
