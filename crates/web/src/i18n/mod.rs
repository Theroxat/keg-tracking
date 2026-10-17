//! Couche de traduction unique.
//!
//! Toute chaîne visible par un utilisateur passe par ici. Deux langues sont
//! livrées, le français (catalogue de référence) et l'anglais ; la langue
//! est choisie **à l'exécution**, requête par requête.
//!
//! La macro [`t!`] résout la clé **à la compilation** contre le catalogue
//! français : une clé absente est une erreur de compilation, pas une chaîne
//! vide découverte en production. Elle renvoie ensuite la valeur dans la
//! langue courante. Que l'anglais possède exactement les mêmes clés, avec
//! les mêmes paramètres, est garanti par un test (`catalogues_alignes`).
//!
//! La langue courante vit dans une variable locale à la tâche Tokio,
//! positionnée par l'intergiciel [`crate::middleware::langue`]. Hors requête
//! (commandes d'exploitation, tests unitaires), c'est le français.

pub mod en;
pub mod fr;

pub use fr::CATALOGUE;

/// Langue d'interface. Fermée : ajouter une variante impose un catalogue
/// de même forme et sa place dans [`Langue::TOUS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Langue {
    #[default]
    Fr,
    En,
}

impl Langue {
    pub const TOUS: &'static [Langue] = &[Langue::Fr, Langue::En];

    /// Code ISO 639-1, tel qu'il est stocké en base, posé dans le cookie
    /// et écrit dans `<html lang>`.
    pub const fn code(self) -> &'static str {
        match self {
            Langue::Fr => "fr",
            Langue::En => "en",
        }
    }

    /// Nom de la langue dans cette langue même : c'est ainsi qu'un
    /// sélecteur reste lisible par qui ne comprend pas la langue affichée.
    pub const fn libelle_natif(self) -> &'static str {
        match self {
            Langue::Fr => "Français",
            Langue::En => "English",
        }
    }

    pub fn depuis_code(code: &str) -> Option<Langue> {
        Langue::TOUS
            .iter()
            .copied()
            .find(|l| l.code().eq_ignore_ascii_case(code.trim()))
    }

    pub fn catalogue(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Langue::Fr => fr::CATALOGUE,
            Langue::En => en::CATALOGUE,
        }
    }

    /// Première langue livrée acceptée par un en-tête `Accept-Language`,
    /// en respectant les poids `q`. `None` si aucune ne convient.
    pub fn depuis_accept_language(entete: &str) -> Option<Langue> {
        let mut meilleure: Option<(Langue, f32)> = None;
        for morceau in entete.split(',') {
            let mut parties = morceau.split(';');
            let etiquette = parties.next().unwrap_or("").trim();
            let poids = parties
                .find_map(|p| p.trim().strip_prefix("q="))
                .and_then(|q| q.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            if poids <= 0.0 {
                continue;
            }
            let primaire = etiquette.split('-').next().unwrap_or("");
            if let Some(l) = Langue::depuis_code(primaire) {
                if meilleure.is_none_or(|(_, q)| poids > q) {
                    meilleure = Some((l, poids));
                }
            }
        }
        meilleure.map(|(l, _)| l)
    }
}

tokio::task_local! {
    static LANGUE: Langue;
}

/// Langue de la requête en cours ; français hors requête.
pub fn langue_courante() -> Langue {
    LANGUE.try_with(|l| *l).unwrap_or_default()
}

/// Exécute `f` (un futur) avec la langue donnée.
pub async fn avec_langue<F: std::future::Future>(langue: Langue, f: F) -> F::Output {
    LANGUE.scope(langue, f).await
}

/// Variante synchrone, pour les tests et le code hors requête.
pub fn avec_langue_sync<R>(langue: Langue, f: impl FnOnce() -> R) -> R {
    LANGUE.sync_scope(langue, f)
}

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

/// Position d'une clé dans le catalogue de référence. Appelée depuis un
/// contexte `const`, une clé inconnue fait échouer la compilation.
pub const fn indice(cle: &str) -> usize {
    let mut i = 0;
    while i < CATALOGUE.len() {
        if octets_egaux(CATALOGUE[i].0, cle) {
            return i;
        }
        i += 1;
    }
    panic!("clé de traduction inconnue — ajoutez-la dans crates/web/src/i18n/fr.rs et en.rs")
}

/// Valeur française d'une clé, résolue à la compilation.
pub const fn chercher(cle: &str) -> &'static str {
    CATALOGUE[indice(cle)].1
}

/// Valeur, dans la langue courante, de la clé française d'indice `i`
/// (calculé à la compilation par [`t!`]). Les catalogues sont dans le même
/// ordre, ce qui rend l'accès direct ; une recherche par clé sert de filet
/// si l'ordre venait à diverger. En dernier recours, le français.
pub fn valeur(i: usize) -> &'static str {
    let (cle, francais) = CATALOGUE[i];
    match langue_courante() {
        Langue::Fr => francais,
        autre => {
            let cat = autre.catalogue();
            match cat.get(i) {
                Some((k, v)) if *k == cle => v,
                _ => cat
                    .iter()
                    .find(|(k, _)| *k == cle)
                    .map(|(_, v)| *v)
                    .unwrap_or(francais),
            }
        }
    }
}

/// Version dynamique, pour les clés calculées (statuts, rôles, erreurs
/// métier). Renvoie un marqueur visible si la clé manque, plutôt qu'une
/// chaîne vide silencieuse.
pub fn traduire(cle: &str) -> &'static str {
    let trouver = |cat: &'static [(&'static str, &'static str)]| {
        cat.iter().find(|(k, _)| *k == cle).map(|(_, v)| *v)
    };
    trouver(langue_courante().catalogue())
        .or_else(|| trouver(CATALOGUE))
        .unwrap_or("⟨traduction manquante⟩")
}

/// `t!("cle")` ou `t!("cle", nom = valeur, …)` pour les messages à trous.
#[macro_export]
macro_rules! t {
    ($cle:literal) => {{
        const INDICE: usize = $crate::i18n::indice($cle);
        $crate::i18n::valeur(INDICE)
    }};
    ($cle:literal, $($nom:ident = $val:expr),+ $(,)?) => {{
        const INDICE: usize = $crate::i18n::indice($cle);
        let mut rendu = $crate::i18n::valeur(INDICE).to_string();
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

    use std::collections::{BTreeMap, BTreeSet, HashSet};

    #[test]
    fn aucune_cle_en_double() {
        for langue in Langue::TOUS {
            let mut vues = HashSet::new();
            for (cle, _) in langue.catalogue() {
                assert!(vues.insert(*cle), "clé en double ({langue:?}) : {cle}");
            }
        }
    }

    #[test]
    fn aucune_valeur_vide() {
        for langue in Langue::TOUS {
            for (cle, val) in langue.catalogue() {
                assert!(
                    !val.trim().is_empty(),
                    "valeur vide ({langue:?}) pour {cle}"
                );
            }
        }
    }

    /// Paramètres `{nom}` d'un message (les spécificateurs de date `%d`
    /// ne sont pas concernés).
    fn parametres(valeur: &str) -> BTreeSet<String> {
        let mut res = BTreeSet::new();
        let mut reste = valeur;
        while let Some(debut) = reste.find('{') {
            let apres = &reste[debut + 1..];
            match apres.find('}') {
                Some(fin) => {
                    res.insert(apres[..fin].to_string());
                    reste = &apres[fin + 1..];
                }
                None => break,
            }
        }
        res
    }

    /// Remplace, pour les langues autres que le français, la vérification
    /// à la compilation : mêmes clés, même ordre, mêmes paramètres.
    #[test]
    fn catalogues_alignes() {
        let reference: BTreeMap<_, _> = CATALOGUE.iter().copied().collect();
        for langue in Langue::TOUS.iter().filter(|l| **l != Langue::Fr) {
            let cat = langue.catalogue();
            let cles: BTreeMap<_, _> = cat.iter().copied().collect();
            let manquantes: Vec<_> = reference
                .keys()
                .filter(|k| !cles.contains_key(*k))
                .collect();
            let en_trop: Vec<_> = cles
                .keys()
                .filter(|k| !reference.contains_key(*k))
                .collect();
            assert!(
                manquantes.is_empty() && en_trop.is_empty(),
                "{langue:?} : clés manquantes {manquantes:?}, clés inconnues du français {en_trop:?}"
            );
            for (cle, fr) in &reference {
                assert_eq!(
                    parametres(fr),
                    parametres(cles[cle]),
                    "{langue:?} : paramètres différents pour {cle}"
                );
                // Les formats de date doivent garder leurs champs.
                if cle.starts_with("temps.date") {
                    assert!(
                        cles[cle].contains("%Y"),
                        "{langue:?} : format {cle} sans année"
                    );
                }
            }
            let ordre_fr: Vec<_> = CATALOGUE.iter().map(|(k, _)| *k).collect();
            let ordre: Vec<_> = cat.iter().map(|(k, _)| *k).collect();
            assert_eq!(
                ordre_fr, ordre,
                "{langue:?} : l'ordre des clés diffère de fr.rs"
            );
        }
    }

    #[test]
    fn t_suit_la_langue_courante() {
        assert_eq!(t!("app.oui"), "Oui");
        let en = avec_langue_sync(Langue::En, || t!("app.oui"));
        assert_eq!(en, en::CATALOGUE[indice("app.oui")].1);
        let en = avec_langue_sync(Langue::En, || traduire("app.oui"));
        assert_eq!(en, en::CATALOGUE[indice("app.oui")].1);
    }

    #[test]
    fn accept_language() {
        let l = Langue::depuis_accept_language;
        assert_eq!(l("en-US,en;q=0.9"), Some(Langue::En));
        assert_eq!(l("de-DE,fr;q=0.5,en;q=0.8"), Some(Langue::En));
        assert_eq!(l("fr-CH, en;q=0.9"), Some(Langue::Fr));
        assert_eq!(l("de, it"), None);
        assert_eq!(l("en;q=0, fr;q=0.1"), Some(Langue::Fr));
        assert_eq!(l(""), None);
    }

    #[test]
    fn codes_de_langue() {
        for l in Langue::TOUS {
            assert_eq!(Langue::depuis_code(l.code()), Some(*l));
        }
        assert_eq!(Langue::depuis_code("xx"), None);
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
