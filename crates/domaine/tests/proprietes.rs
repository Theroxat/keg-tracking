//! Test basé sur les propriétés de la machine à états et des projections.
//!
//! Plutôt que d'énumérer des cas, on engendre des milliers d'histoires
//! valides et on vérifie des invariants qui doivent tenir pour toutes.

use chrono::{DateTime, Duration, Utc};
use domaine::evenement::EvenementApplique;
use domaine::projection::{appliquer, reconstruire};
use domaine::{EtatFut, Statut, TypeEvenement};
use proptest::prelude::*;
use uuid::Uuid;

const BIERE: Uuid = Uuid::from_u128(1);
const LOT: Uuid = Uuid::from_u128(2);
const CUVE: Uuid = Uuid::from_u128(3);
const CLIENT: Uuid = Uuid::from_u128(4);
const ADRESSE: Uuid = Uuid::from_u128(5);
const EMPLACEMENT: Uuid = Uuid::from_u128(6);

/// Construit une histoire **valide** : chaque étape est tirée parmi les
/// transitions réellement autorisées depuis l'état courant.
fn histoire(choix: Vec<u8>) -> Vec<EvenementApplique> {
    let base: DateTime<Utc> = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    let mut statut = Statut::Recu;
    let mut evenements = vec![EvenementApplique {
        id: Uuid::from_u128(1000),
        sequence: 1,
        type_evenement: TypeEvenement::MiseEnService,
        statut_apres: Some(statut),
        emplacement_id: Some(EMPLACEMENT),
        client_id: None,
        adresse_client_id: None,
        type_biere_id: None,
        lot_id: None,
        cuve_id: None,
        utilisateur_id: Some(Uuid::from_u128(9)),
        survenu_le: base,
    }];

    for (i, c) in choix.iter().enumerate() {
        let possibles = statut.transitions_valides();
        let suivant = possibles[*c as usize % possibles.len()];
        let mut e = EvenementApplique {
            id: Uuid::from_u128(1001 + i as u128),
            sequence: evenements.len() as i64 + 1,
            type_evenement: TypeEvenement::Transition,
            statut_apres: Some(suivant),
            emplacement_id: None,
            client_id: None,
            adresse_client_id: None,
            type_biere_id: None,
            lot_id: None,
            cuve_id: None,
            utilisateur_id: Some(Uuid::from_u128(9)),
            survenu_le: base + Duration::hours(i as i64 + 1),
        };
        match suivant {
            Statut::Rempli => {
                e.type_biere_id = Some(BIERE);
                e.lot_id = Some(LOT);
                e.cuve_id = Some(CUVE);
            }
            Statut::Livre => {
                e.client_id = Some(CLIENT);
                e.adresse_client_id = Some(ADRESSE);
            }
            _ => e.emplacement_id = Some(EMPLACEMENT),
        }
        evenements.push(e);
        statut = suivant;
    }
    evenements
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// Le pli n'a pas d'état caché : rejouer depuis n'importe quel point de
    /// coupure donne le même résultat. C'est ce qui rend la commande de
    /// reconstruction sûre.
    #[test]
    fn le_rejeu_est_independant_du_point_de_coupure(
        choix in prop::collection::vec(any::<u8>(), 0..40),
        coupure in 0usize..40,
    ) {
        let ev = histoire(choix);
        let n = ev.len();
        let k = coupure.min(n);
        let complet = reconstruire(&ev);
        let partiel = reconstruire(&ev[..k]);
        let repris = ev[k..].iter().fold(partiel, appliquer);
        prop_assert_eq!(complet, repris);
    }

    /// L'état projeté suit toujours le dernier événement.
    #[test]
    fn le_statut_projete_est_celui_du_dernier_evenement(
        choix in prop::collection::vec(any::<u8>(), 0..40),
    ) {
        let ev = histoire(choix);
        let etat = reconstruire(&ev);
        prop_assert_eq!(etat.statut, ev.last().unwrap().statut_apres);
        prop_assert_eq!(etat.sequence_courante, ev.len() as i64);
        prop_assert_eq!(etat.dernier_evenement_id, Some(ev.last().unwrap().id));
    }

    /// Toutes les transitions engendrées sont légales : le générateur ne
    /// peut pas produire d'histoire impossible, et la table de transitions
    /// n'a donc aucun trou.
    #[test]
    fn toute_histoire_engendree_est_legale(
        choix in prop::collection::vec(any::<u8>(), 0..40),
    ) {
        let ev = histoire(choix);
        for paire in ev.windows(2) {
            let avant = paire[0].statut_apres.unwrap();
            let apres = paire[1].statut_apres.unwrap();
            prop_assert!(
                avant.peut_aller_vers(apres),
                "{:?} -> {:?} ne devrait pas être engendrée", avant, apres
            );
        }
    }

    /// Invariants métier : un fût sale est vide, un fût livré n'est nulle
    /// part dans la brasserie, un fût rempli a toujours son contenu.
    #[test]
    fn les_invariants_metier_tiennent(
        choix in prop::collection::vec(any::<u8>(), 0..40),
    ) {
        let ev = histoire(choix);
        let mut etat = EtatFut::default();
        for e in &ev {
            etat = appliquer(etat, e);
            match e.statut_apres.unwrap() {
                Statut::Sale => {
                    prop_assert!(etat.lot_id.is_none(), "un fût sale ne contient rien");
                    prop_assert!(etat.type_biere_id.is_none());
                    prop_assert!(etat.date_remplissage.is_none());
                    prop_assert!(etat.client_id.is_none());
                }
                Statut::Rempli => {
                    prop_assert_eq!(etat.lot_id, Some(LOT));
                    prop_assert!(etat.date_remplissage.is_some());
                }
                Statut::Livre => {
                    prop_assert_eq!(etat.client_id, Some(CLIENT));
                    prop_assert_eq!(etat.adresse_client_id, Some(ADRESSE));
                    prop_assert!(
                        etat.emplacement_id.is_none(),
                        "un fût chez un client n'a pas d'emplacement dans la brasserie"
                    );
                }
                Statut::Propre => {
                    prop_assert!(etat.dernier_lavage_le.is_some(), "le lavage doit être tracé");
                    prop_assert_eq!(etat.dernier_lavage_par, e.utilisateur_id);
                }
                Statut::Recu => prop_assert!(etat.client_id.is_none()),
                _ => {}
            }
        }
    }

    /// `statut_depuis_le` date bien le dernier **changement** de statut, et
    /// pas la dernière écriture : c'est ce qui fait tenir l'alerte
    /// « fût non revenu depuis N jours ».
    #[test]
    fn l_anciennete_date_le_dernier_changement(
        choix in prop::collection::vec(any::<u8>(), 1..40),
    ) {
        let ev = histoire(choix);
        let etat = reconstruire(&ev);
        let dernier_changement = ev
            .iter()
            .rev()
            .zip(ev.iter().rev().skip(1).map(Some).chain(std::iter::once(None)))
            .find(|(e, precedent)| {
                precedent.map(|p| p.statut_apres != e.statut_apres).unwrap_or(true)
            })
            .map(|(e, _)| e.survenu_le);
        prop_assert_eq!(etat.statut_depuis_le, dernier_changement);
    }
}
