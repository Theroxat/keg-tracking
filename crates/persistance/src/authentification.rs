use argon2::password_hash::{rand_core::OsRng, PasswordHash, SaltString};
use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::{Erreur, Resultat};

/// Hachage des mots de passe et des codes PIN.
///
/// Argon2id aux paramètres recommandés par l'OWASP, **poivré** avec un secret
/// d'application : une base volée ne suffit pas à attaquer les mots de passe
/// hors ligne. Changer le secret invalide tous les mots de passe — c'est
/// documenté dans `docs/exploitation.md`.
#[derive(Clone)]
pub struct Hacheur {
    secret: Vec<u8>,
}

const MEMOIRE_KIO: u32 = 19_456;
const ITERATIONS: u32 = 2;
const PARALLELISME: u32 = 1;

impl Hacheur {
    pub fn nouveau(secret: &[u8]) -> Resultat<Hacheur> {
        if secret.len() < 32 {
            return Err(Erreur::Configuration(
                "SECRET_APP doit faire au moins 32 octets".into(),
            ));
        }
        Ok(Hacheur {
            secret: secret.to_vec(),
        })
    }

    fn argon(&self) -> Resultat<Argon2<'_>> {
        let params = Params::new(MEMOIRE_KIO, ITERATIONS, PARALLELISME, None)
            .map_err(|e| Erreur::Hachage(e.to_string()))?;
        Argon2::new_with_secret(&self.secret, Algorithm::Argon2id, Version::V0x13, params)
            .map_err(|e| Erreur::Hachage(e.to_string()))
    }

    pub fn hacher(&self, clair: &str) -> Resultat<String> {
        let sel = SaltString::generate(&mut OsRng);
        Ok(self
            .argon()?
            .hash_password(clair.as_bytes(), &sel)
            .map_err(|e| Erreur::Hachage(e.to_string()))?
            .to_string())
    }

    /// Vérification à temps constant côté Argon2. Renvoie `false` sur un
    /// hachage illisible plutôt que de propager une erreur exploitable.
    pub fn verifier(&self, clair: &str, hachage: &str) -> bool {
        let Ok(argon) = self.argon() else {
            return false;
        };
        let Ok(attendu) = PasswordHash::new(hachage) else {
            return false;
        };
        argon.verify_password(clair.as_bytes(), &attendu).is_ok()
    }
}

/// Jeton opaque de 32 octets, rendu en base64 URL.
pub fn jeton_aleatoire() -> String {
    let mut octets = [0u8; 32];
    OsRng.fill_bytes(&mut octets);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(octets)
}

/// Empreinte stockée en base : un vol de la table `session` ne donne aucune
/// session utilisable.
pub fn empreinte(jeton: &str) -> Vec<u8> {
    Sha256::digest(jeton.as_bytes()).to_vec()
}

/// Exigences minimales sur un mot de passe. Volontairement simples et
/// explicables : longueur d'abord, c'est ce qui compte.
pub fn mot_de_passe_acceptable(mdp: &str) -> Result<(), &'static str> {
    if mdp.chars().count() < 12 {
        return Err("mdp.trop_court");
    }
    if mdp.chars().count() > 256 {
        return Err("mdp.trop_long");
    }
    let classes = [
        mdp.chars().any(|c| c.is_lowercase()),
        mdp.chars().any(|c| c.is_uppercase()),
        mdp.chars().any(|c| c.is_ascii_digit()),
        mdp.chars().any(|c| !c.is_alphanumeric()),
    ]
    .iter()
    .filter(|x| **x)
    .count();
    if classes < 3 {
        return Err("mdp.trop_simple");
    }
    Ok(())
}

pub fn pin_acceptable(pin: &str) -> Result<(), &'static str> {
    if pin.len() != 6 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err("pin.format");
    }
    if pin.as_bytes().windows(2).all(|p| p[0] == p[1]) {
        return Err("pin.trop_simple");
    }
    let croissant = pin.as_bytes().windows(2).all(|p| p[1] == p[0] + 1);
    let decroissant = pin.as_bytes().windows(2).all(|p| p[0] == p[1] + 1);
    if croissant || decroissant {
        return Err("pin.trop_simple");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h() -> Hacheur {
        Hacheur::nouveau(b"0123456789abcdef0123456789abcdef").unwrap()
    }

    #[test]
    fn aller_retour_de_hachage() {
        let hh = h();
        let hachage = hh.hacher("correct horse battery staple").unwrap();
        assert!(hh.verifier("correct horse battery staple", &hachage));
        assert!(!hh.verifier("mauvais", &hachage));
    }

    #[test]
    fn le_poivre_change_tout() {
        let a = Hacheur::nouveau(b"0123456789abcdef0123456789abcdef").unwrap();
        let b = Hacheur::nouveau(b"ffffffffffffffffffffffffffffffff").unwrap();
        let hachage = a.hacher("motdepasse").unwrap();
        assert!(a.verifier("motdepasse", &hachage));
        assert!(
            !b.verifier("motdepasse", &hachage),
            "un autre secret ne doit pas valider"
        );
    }

    #[test]
    fn jetons_distincts_et_empreinte_stable() {
        let a = jeton_aleatoire();
        let b = jeton_aleatoire();
        assert_ne!(a, b);
        assert_eq!(empreinte(&a), empreinte(&a));
        assert_ne!(empreinte(&a), empreinte(&b));
        assert_eq!(empreinte(&a).len(), 32);
    }

    #[test]
    fn regles_mot_de_passe() {
        assert!(mot_de_passe_acceptable("Brasserie2026!").is_ok());
        assert_eq!(mot_de_passe_acceptable("court1!A"), Err("mdp.trop_court"));
        assert_eq!(
            mot_de_passe_acceptable("tropsimplemaislong"),
            Err("mdp.trop_simple")
        );
    }

    #[test]
    fn regles_pin() {
        assert!(pin_acceptable("407913").is_ok());
        assert_eq!(pin_acceptable("12345"), Err("pin.format"));
        assert_eq!(pin_acceptable("abcdef"), Err("pin.format"));
        assert_eq!(pin_acceptable("111111"), Err("pin.trop_simple"));
        assert_eq!(pin_acceptable("123456"), Err("pin.trop_simple"));
        assert_eq!(pin_acceptable("654321"), Err("pin.trop_simple"));
    }
}
