pub mod exporter;
pub mod provisionner;
pub mod reconstruire;
pub mod semer;
pub mod serve;

use anyhow::Context;
use persistance::authentification::Hacheur;
use persistance::Depot;
use web::Config;

/// Charge la configuration et échoue **bruyamment** si une variable
/// obligatoire manque — en les listant toutes d'un coup, pas une par
/// redémarrage.
pub fn config() -> anyhow::Result<Config> {
    Config::depuis_env().map_err(|manquantes| {
        anyhow::anyhow!(
            "configuration incomplète. Variables d'environnement manquantes ou invalides :\n  - {}",
            manquantes.join("\n  - ")
        )
    })
}

pub async fn connecter(config: &Config) -> anyhow::Result<(Depot, Hacheur)> {
    let depot = Depot::connecter(
        &config.url_app,
        &config.url_editeur,
        &config.url_auth,
        config.connexions_max,
    )
    .await
    .context("connexion à PostgreSQL")?;
    let hacheur = Hacheur::nouveau(config.secret_app.as_bytes())
        .map_err(|e| anyhow::anyhow!("secret applicatif : {e}"))?;
    Ok((depot, hacheur))
}

/// Mot de passe fort généré, lisible à l'oral pour être transmis par
/// téléphone sans ambiguïté.
pub fn mot_de_passe_genere() -> String {
    use rand::Rng;
    const MOTS: &[&str] = &[
        "houblon", "levure", "brassin", "malte", "cuve", "garde", "trempe", "filtre", "soutire",
        "ambree", "blonde", "triple", "saison", "stout", "porter", "gose", "fermente", "empatage",
        "rincage", "drache", "moulin", "chaude", "froide", "tirage",
    ];
    let mut rng = rand::thread_rng();
    let mots: Vec<&str> = (0..4).map(|_| MOTS[rng.gen_range(0..MOTS.len())]).collect();
    format!("{}-{}{}", mots.join("-"), rng.gen_range(10..99), "!A")
}
