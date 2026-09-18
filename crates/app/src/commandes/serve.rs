use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use web::etat::Debit;
use web::Etat;

use super::{config, connecter};

pub async fn executer() -> anyhow::Result<()> {
    let config = config()?;

    // Migrations jouées avec le rôle propriétaire, avant l'ouverture du port.
    tracing::info!("migrations en cours");
    persistance::garde::migrer(&config.url_owner)
        .await
        .context("migrations")?;
    tracing::info!("migrations à jour");

    let (depot, hacheur) = connecter(&config).await?;
    let adresse = config.adresse_ecoute;
    let etat = Etat {
        depot: depot.clone(),
        hacheur,
        config: Arc::new(config),
        debit: Arc::new(Debit::nouveau()),
    };

    // Ménage périodique : seaux de débit en mémoire et sessions périmées.
    {
        let etat = etat.clone();
        tokio::spawn(async move {
            let mut tic = tokio::time::interval(Duration::from_secs(600));
            loop {
                tic.tick().await;
                etat.debit.nettoyer(Duration::from_secs(3600));
                match persistance::session::purger(etat.depot.auth()).await {
                    Ok(n) if n > 0 => tracing::info!(sessions = n, "sessions purgées"),
                    Err(e) => tracing::warn!(erreur = %e, "purge des sessions"),
                    _ => {}
                }
            }
        });
    }

    let app = web::routeur(etat);
    let ecoute = tokio::net::TcpListener::bind(adresse)
        .await
        .with_context(|| format!("écoute sur {adresse}"))?;
    tracing::info!(%adresse, "serveur démarré");

    axum::serve(
        ecoute,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(arret())
    .await
    .context("serveur")?;

    depot.fermer().await;
    tracing::info!("arrêt propre");
    Ok(())
}

async fn arret() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminaison = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminaison = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminaison => {},
    }
    tracing::info!("signal d'arrêt reçu");
}
