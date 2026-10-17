//! Couche de traduction unique.
//!
//! Toute chaîne visible par un utilisateur passe par ici. La seule langue
//! livrée est le français, mais la centralisation est faite dès le départ :
//! ajouter une langue devient mécanique au lieu d'être une chasse au trésor
//! dans les gabarits.
//!
//! La macro [`t!`] résout la clé **à la compilation** : une clé absente est
//! une erreur de compilation, pas une chaîne vide découverte en production.

pub mod fr;

pub use fr::CATALOGUE;

const fn octets_egaux(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Recherche constante dans le catalogue. Appelée depuis un contexte `const`,
/// une clé inconnue fait échouer la compilation.
pub const fn chercher(cle: &str) -> &'static str {
    let mut i = 0;
    while i < CATALOGUE.len() {
        if octets_egaux(CATALOGUE[i].0, cle) {
            return CATALOGUE[i].1;
        }
        i += 1;
    }
    panic!("clé de traduction inconnue — ajoutez-la dans crates/web/src/i18n/fr.rs")
}

/// Version dynamique, pour les clés calculées (statuts, rôles, erreurs
/// métier). Renvoie la clé elle-même si elle manque, ce qui la rend visible
/// à l'écran plutôt que silencieuse.
pub fn traduire(cle: &str) -> &'static str {
    CATALOGUE
        .iter()
        .find(|(k, _)| *k == cle)
        .map(|(_, v)| *v)
        .unwrap_or("⟨traduction manquante⟩")
}

/// `t!("cle")` ou `t!("cle", nom = valeur, …)` pour les messages à trous.
#[macro_export]
macro_rules! t {
    ($cle:literal) => {{
        const VALEUR: &str = $crate::i18n::chercher($cle);
        VALEUR
    }};
    ($cle:literal, $($nom:ident = $val:expr),+ $(,)?) => {{
        const VALEUR: &str = $crate::i18n::chercher($cle);
        let mut rendu = VALEUR.to_string();
        $(
            rendu = rendu.replace(
                concat!("{", stringify!($nom), "}"),
                &$val.to_string(),
            );
        )+
        rendu
    }};
}

/// Message d'une erreur métier, avec ses paramètres.
pub fn message_erreur(e: &domaine::ErreurMetier) -> String {
    use domaine::ErreurMetier as E;
    let gabarit = traduire(e.cle_i18n());
    match e {
        E::TransitionInterdite { avant, apres } => gabarit
            .replace("{avant}", &traduire(avant.cle_i18n()).to_lowercase())
            .replace("{apres}", &traduire(apres.cle_i18n()).to_lowercase()),
        E::DejaDansCeStatut { statut } => gabarit.replace("{statut}", traduire(statut.cle_i18n())),
        E::PreconditionManquante { statut, champ } => gabarit
            .replace("{statut}", traduire(statut.cle_i18n()))
            .replace("{champ}", traduire(&format!("champ.{champ}"))),
        E::ContenanceNonAutorisee { litres } => gabarit.replace("{litres}", &litres.to_string()),
        E::CodeQrInconnu { code } => gabarit.replace("{code}", code),
        _ => gabarit.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aucune_cle_en_double() {
        let mut vues = std::collections::HashSet::new();
        for (cle, _) in CATALOGUE {
            assert!(vues.insert(*cle), "clé en double : {cle}");
        }
    }

    #[test]
    fn aucune_valeur_vide() {
        for (cle, val) in CATALOGUE {
            assert!(!val.trim().is_empty(), "valeur vide pour {cle}");
        }
    }

    /// Tout statut, rôle, type d'événement et erreur métier doit avoir sa
    /// traduction : sinon un badge s'affiche vide en production.
    #[test]
    fn toutes_les_cles_des_types_fermes_existent() {
        for s in domaine::Statut::TOUS {
            assert_ne!(traduire(s.cle_i18n()), "⟨traduction manquante⟩", "{:?}", s);
        }
        for r in domaine::Role::TOUS {
            assert_ne!(traduire(r.cle_i18n()), "⟨traduction manquante⟩", "{:?}", r);
        }
        for t in domaine::TypeEvenement::TOUS {
            assert_ne!(traduire(t.cle_i18n()), "⟨traduction manquante⟩", "{:?}", t);
        }
        for d in domaine::TypeDocument::TOUS {
            assert_ne!(traduire(d.cle_i18n()), "⟨traduction manquante⟩", "{:?}", d);
        }
    }
}
