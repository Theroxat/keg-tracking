use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use persistance::authentification::Hacheur;
use persistance::Depot;

/// Configuration, exclusivement par variables d'environnement.
/// **Zéro secret dans le code.**
#[derive(Debug, Clone)]
pub struct Config {
    pub url_owner: String,
    pub url_app: String,
    pub url_editeur: String,
    pub url_auth: String,
    pub secret_app: String,
    pub adresse_ecoute: SocketAddr,
    pub domaine_public: String,
    pub fuseau_defaut: String,
    /// Derrière un proxy de confiance (Caddy, tunnel Cloudflare) : l'adresse
    /// réelle du client se lit dans les en-têtes transmis.
    pub derriere_proxy: bool,
    /// Le navigateur voit du HTTPS même si l'application reçoit du HTTP :
    /// c'est le cas derrière un tunnel. Les cookies restent `Secure`.
    pub cookies_securises: bool,
    pub connexions_max: u32,
}

fn lire(nom: &str, manquantes: &mut Vec<String>) -> String {
    match std::env::var(nom) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            manquantes.push(nom.to_string());
            String::new()
        }
    }
}

fn lire_ou(nom: &str, defaut: &str) -> String {
    std::env::var(nom)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| defaut.to_string())
}

fn booleen(nom: &str, defaut: bool) -> bool {
    match std::env::var(nom).ok().as_deref() {
        Some("1") | Some("true") | Some("oui") => true,
        Some("0") | Some("false") | Some("non") => false,
        _ => defaut,
    }
}

impl Config {
    /// Collecte **toutes** les variables manquantes et les signale ensemble.
    /// Une par redémarrage serait une perte de temps à l'installation.
    pub fn depuis_env() -> Result<Config, Vec<String>> {
        let mut manquantes = Vec::new();

        let url_owner = lire("DATABASE_URL_OWNER", &mut manquantes);
        let url_app = lire("DATABASE_URL_APP", &mut manquantes);
        let url_editeur = lire("DATABASE_URL_EDITEUR", &mut manquantes);
        let url_auth = lire("DATABASE_URL_AUTH", &mut manquantes);
        let secret_app = lire("SECRET_APP", &mut manquantes);
        let domaine_public = lire("DOMAINE_PUBLIC", &mut manquantes);

        if !secret_app.is_empty() && secret_app.len() < 32 {
            manquantes.push("SECRET_APP (au moins 32 caractères)".into());
        }

        let ecoute = lire_ou("ADRESSE_ECOUTE", "0.0.0.0:8080");
        let adresse_ecoute = match ecoute.parse::<SocketAddr>() {
            Ok(a) => a,
            Err(_) => {
                manquantes.push(format!("ADRESSE_ECOUTE (valeur illisible : {ecoute})"));
                "0.0.0.0:8080".parse().unwrap()
            }
        };

        if !manquantes.is_empty() {
            return Err(manquantes);
        }

        Ok(Config {
            url_owner,
            url_app,
            url_editeur,
            url_auth,
            secret_app,
            adresse_ecoute,
            domaine_public,
            fuseau_defaut: lire_ou("FUSEAU_DEFAUT", "Europe/Paris"),
            derriere_proxy: booleen("DERRIERE_PROXY", true),
            cookies_securises: booleen("COOKIES_SECURISES", true),
            connexions_max: lire_ou("CONNEXIONS_MAX", "16").parse().unwrap_or(16),
        })
    }

    pub fn url_publique(&self) -> String {
        format!("https://{}", self.domaine_public)
    }
}

/// Limitation de débit par fenêtre glissante, en mémoire.
///
/// Suffisant pour une instance unique, qui est le mode de déploiement v1.
/// Le comptage **par compte** vit en base et survit à un redémarrage : c'est
/// lui qui protège réellement contre le bourrage de mots de passe.
pub struct Debit {
    seaux: Mutex<HashMap<String, Vec<Instant>>>,
}

impl Debit {
    pub fn nouveau() -> Debit {
        Debit {
            seaux: Mutex::new(HashMap::new()),
        }
    }

    pub fn autoriser(&self, cle: &str, max: usize, fenetre: Duration) -> bool {
        let maintenant = Instant::now();
        let mut seaux = match self.seaux.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let entree = seaux.entry(cle.to_string()).or_default();
        entree.retain(|t| maintenant.duration_since(*t) < fenetre);
        if entree.len() >= max {
            return false;
        }
        entree.push(maintenant);
        true
    }

    /// Ménage : sans cela la table grossit indéfiniment sur un service exposé.
    pub fn nettoyer(&self, plus_vieux_que: Duration) {
        let maintenant = Instant::now();
        let mut seaux = match self.seaux.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        seaux.retain(|_, v| {
            v.retain(|t| maintenant.duration_since(*t) < plus_vieux_que);
            !v.is_empty()
        });
    }
}

#[derive(Clone)]
pub struct Etat {
    pub depot: Depot,
    pub hacheur: Hacheur,
    pub config: Arc<Config>,
    pub debit: Arc<Debit>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_debit_bloque_au_dela_du_seuil() {
        let d = Debit::nouveau();
        let f = Duration::from_secs(60);
        for _ in 0..3 {
            assert!(d.autoriser("ip", 3, f));
        }
        assert!(
            !d.autoriser("ip", 3, f),
            "la 4e tentative doit être refusée"
        );
        assert!(d.autoriser("autre-ip", 3, f), "les clés sont indépendantes");
    }

    #[test]
    fn le_nettoyage_vide_les_seaux_perimes() {
        let d = Debit::nouveau();
        assert!(d.autoriser("ip", 1, Duration::from_millis(1)));
        std::thread::sleep(Duration::from_millis(5));
        d.nettoyer(Duration::from_millis(1));
        assert!(d.autoriser("ip", 1, Duration::from_millis(1)));
    }
}
