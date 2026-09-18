use anyhow::{bail, Context};
use clap::Args as ClapArgs;
use domaine::Role;
use persistance::authentification::mot_de_passe_acceptable;

use super::{config, connecter, mot_de_passe_genere};

#[derive(ClapArgs)]
pub struct Args {
    /// Nom affiché de la brasserie.
    #[arg(long)]
    pub nom: String,
    /// Identifiant court, en minuscules, sans espace.
    #[arg(long)]
    pub slug: String,
    /// Préfixe des codes QR, en majuscules. Unique sur toute l'installation.
    #[arg(long)]
    pub prefixe_qr: String,
    /// Adresse e-mail du premier administrateur.
    #[arg(long)]
    pub admin_email: String,
    #[arg(long, default_value = "Administrateur")]
    pub admin_nom: String,
    #[arg(long, default_value = "")]
    pub admin_prenom: String,
    /// Mot de passe imposé. Omis, un mot de passe fort est généré.
    #[arg(long, env = "ADMIN_MOT_DE_PASSE")]
    pub admin_mot_de_passe: Option<String>,
    #[arg(long, default_value = "Europe/Paris")]
    pub fuseau: String,
}

pub async fn brasserie(a: Args) -> anyhow::Result<()> {
    let config = config()?;
    let (depot, hacheur) = connecter(&config).await?;

    let mot_de_passe = a
        .admin_mot_de_passe
        .clone()
        .unwrap_or_else(mot_de_passe_genere);
    if let Err(cle) = mot_de_passe_acceptable(&mot_de_passe) {
        bail!(
            "mot de passe refusé ({cle}) : au moins 12 caractères et trois classes de caractères"
        );
    }

    let mut tx = depot.ouvrir_editeur().await?;
    let locataire_id = persistance::locataire::creer(
        tx.conn(),
        &persistance::locataire::NouveauLocataire {
            nom: a.nom.clone(),
            identifiant_court: a.slug.to_lowercase(),
            prefixe_qr: a.prefixe_qr.to_uppercase(),
            fuseau_horaire: a.fuseau.clone(),
        },
    )
    .await
    .context("création de la brasserie (slug ou préfixe QR déjà pris ?)")?;

    let hachage = hacheur
        .hacher(&mot_de_passe)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let admin_id = persistance::utilisateur::creer(
        tx.conn(),
        &persistance::utilisateur::NouvelUtilisateur {
            tenant_id: locataire_id,
            email: &a.admin_email.trim().to_lowercase(),
            mot_de_passe_hash: &hachage,
            nom: &a.admin_nom,
            prenom: &a.admin_prenom,
            role: Role::Administrateur,
        },
    )
    .await
    .context("création de l'administrateur (adresse déjà utilisée ?)")?;

    // Référentiels minimaux, pour que l'application soit utilisable tout de
    // suite : sans emplacement, aucune opération n'est possible.
    sqlx::query(
        "insert into emplacement (tenant_id, nom) values ($1,'Brasserie'),($1,'Zone de lavage'),($1,'Quai')",
    )
    .bind(locataire_id)
    .execute(tx.conn())
    .await?;
    let defaut: uuid::Uuid =
        sqlx::query_scalar("select id from emplacement where tenant_id = $1 and nom = 'Brasserie'")
            .bind(locataire_id)
            .fetch_one(tx.conn())
            .await?;
    sqlx::query("update locataire set emplacement_defaut_id = $2 where id = $1")
        .bind(locataire_id)
        .bind(defaut)
        .execute(tx.conn())
        .await?;

    persistance::super_admin::tracer(
        tx.conn(),
        None,
        "brasserie.creee",
        Some(locataire_id),
        serde_json::json!({ "slug": a.slug, "admin": a.admin_email }),
        None,
    )
    .await?;
    tx.valider().await?;

    println!("\nBrasserie créée.");
    println!("  nom          : {}", a.nom);
    println!("  identifiant  : {}", a.slug.to_lowercase());
    println!("  préfixe QR   : {}", a.prefixe_qr.to_uppercase());
    println!("  administrateur : {} ({admin_id})", a.admin_email);
    if a.admin_mot_de_passe.is_none() {
        println!("\n  MOT DE PASSE : {mot_de_passe}");
        println!("  Communiquez-le par un canal sûr. Il ne sera plus affiché.");
    }
    println!("\n  Connexion : {}\n", config.url_publique());
    depot.fermer().await;
    Ok(())
}

#[derive(ClapArgs)]
pub struct ArgsEditeur {
    #[arg(long)]
    pub email: String,
    #[arg(long, default_value = "Éditeur")]
    pub nom: String,
    #[arg(long, env = "EDITEUR_MOT_DE_PASSE")]
    pub mot_de_passe: Option<String>,
}

pub async fn editeur(a: ArgsEditeur) -> anyhow::Result<()> {
    let config = config()?;
    let (depot, hacheur) = connecter(&config).await?;
    let mot_de_passe = a.mot_de_passe.clone().unwrap_or_else(mot_de_passe_genere);
    if let Err(cle) = mot_de_passe_acceptable(&mot_de_passe) {
        bail!("mot de passe refusé ({cle})");
    }
    let hachage = hacheur
        .hacher(&mot_de_passe)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut tx = depot.ouvrir_editeur().await?;
    let id = persistance::super_admin::creer(
        tx.conn(),
        &a.email.trim().to_lowercase(),
        &hachage,
        &a.nom,
    )
    .await
    .context("création du compte éditeur")?;
    tx.valider().await?;
    println!("Compte éditeur créé : {} ({id})", a.email);
    if a.mot_de_passe.is_none() {
        println!("MOT DE PASSE : {mot_de_passe}");
    }
    depot.fermer().await;
    Ok(())
}
