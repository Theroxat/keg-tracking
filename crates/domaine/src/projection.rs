use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::evenement::EvenementApplique;
use crate::statut::Statut;

/// État courant d'un fût. C'est un cache : la vérité est la suite des
/// événements. `reconstruire` doit toujours redonner exactement cette valeur.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EtatFut {
    pub statut: Option<Statut>,
    pub emplacement_id: Option<Uuid>,
    pub client_id: Option<Uuid>,
    pub adresse_client_id: Option<Uuid>,
    pub type_biere_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub cuve_id: Option<Uuid>,
    pub date_remplissage: Option<DateTime<Utc>>,
    pub statut_depuis_le: Option<DateTime<Utc>>,
    pub dernier_lavage_le: Option<DateTime<Utc>>,
    pub dernier_lavage_par: Option<Uuid>,
    pub dernier_evenement_id: Option<Uuid>,
    pub dernier_evenement_le: Option<DateTime<Utc>>,
    pub sequence_courante: i64,
}

/// Le pli : état + événement -> nouvel état. Pure, sans entrée/sortie.
/// Utilisée à l'écriture (incrémentale) comme à la reconstruction (rejeu).
pub fn appliquer(mut etat: EtatFut, e: &EvenementApplique) -> EtatFut {
    etat.sequence_courante = e.sequence;
    etat.dernier_evenement_id = Some(e.id);
    etat.dernier_evenement_le = Some(e.survenu_le);

    if e.emplacement_id.is_some() {
        etat.emplacement_id = e.emplacement_id;
    }

    let Some(apres) = e.statut_apres else {
        // Événement sans changement de statut : on ne pose que ce qui est
        // explicitement fourni.
        if e.type_biere_id.is_some() {
            etat.type_biere_id = e.type_biere_id;
        }
        if e.lot_id.is_some() {
            etat.lot_id = e.lot_id;
        }
        if e.cuve_id.is_some() {
            etat.cuve_id = e.cuve_id;
        }
        if e.client_id.is_some() {
            etat.client_id = e.client_id;
            etat.adresse_client_id = e.adresse_client_id;
        }
        return etat;
    };

    if etat.statut != Some(apres) {
        etat.statut_depuis_le = Some(e.survenu_le);
    }
    etat.statut = Some(apres);

    match apres {
        // De retour à la brasserie, pas encore trié. Le contenu est conservé :
        // un fût peut revenir plein.
        Statut::Recu => {
            etat.client_id = None;
            etat.adresse_client_id = None;
        }
        // Le fût est vide et souillé : plus de contenu, plus de client.
        Statut::Sale => {
            etat.client_id = None;
            etat.adresse_client_id = None;
            etat.type_biere_id = None;
            etat.lot_id = None;
            etat.cuve_id = None;
            etat.date_remplissage = None;
        }
        // Traçabilité sanitaire : qui a lavé, et quand.
        Statut::Propre => {
            etat.dernier_lavage_le = Some(e.survenu_le);
            etat.dernier_lavage_par = e.utilisateur_id;
            etat.client_id = None;
            etat.adresse_client_id = None;
        }
        Statut::Rempli => {
            etat.type_biere_id = e.type_biere_id;
            etat.lot_id = e.lot_id;
            etat.cuve_id = e.cuve_id;
            etat.date_remplissage = Some(e.survenu_le);
        }
        // Chez le client : il n'est plus à un emplacement de la brasserie.
        Statut::Livre => {
            etat.client_id = e.client_id;
            etat.adresse_client_id = e.adresse_client_id;
            etat.emplacement_id = None;
        }
        // Sorti du parc : on ne touche à rien, l'histoire reste lisible.
        Statut::HorsService | Statut::Perdu => {}
    }

    etat
}

/// Rejoue une suite d'événements depuis l'état initial.
/// Les événements doivent être triés par `sequence` croissante.
pub fn reconstruire(evenements: &[EvenementApplique]) -> EtatFut {
    evenements.iter().fold(EtatFut::default(), appliquer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evenement::TypeEvenement;

    fn ev(seq: i64, statut: Option<Statut>) -> EvenementApplique {
        EvenementApplique {
            id: Uuid::from_u128(seq as u128),
            sequence: seq,
            type_evenement: TypeEvenement::Transition,
            statut_apres: statut,
            emplacement_id: None,
            client_id: None,
            adresse_client_id: None,
            type_biere_id: None,
            lot_id: None,
            cuve_id: None,
            utilisateur_id: Some(Uuid::nil()),
            survenu_le: DateTime::from_timestamp(1_700_000_000 + seq * 3600, 0).unwrap(),
        }
    }

    #[test]
    fn le_passage_en_sale_vide_le_contenu() {
        let mut rempli = ev(1, Some(Statut::Rempli));
        rempli.type_biere_id = Some(Uuid::from_u128(10));
        rempli.lot_id = Some(Uuid::from_u128(11));
        rempli.cuve_id = Some(Uuid::from_u128(12));

        let etat = appliquer(EtatFut::default(), &rempli);
        assert_eq!(etat.lot_id, Some(Uuid::from_u128(11)));
        assert!(etat.date_remplissage.is_some());

        let etat = appliquer(etat, &ev(2, Some(Statut::Sale)));
        assert_eq!(etat.lot_id, None);
        assert_eq!(etat.type_biere_id, None);
        assert_eq!(etat.date_remplissage, None);
    }

    #[test]
    fn un_fut_peut_revenir_plein() {
        let mut rempli = ev(1, Some(Statut::Rempli));
        rempli.type_biere_id = Some(Uuid::from_u128(10));
        rempli.lot_id = Some(Uuid::from_u128(11));
        rempli.cuve_id = Some(Uuid::from_u128(12));
        let etat = appliquer(EtatFut::default(), &rempli);

        let mut livre = ev(2, Some(Statut::Livre));
        livre.client_id = Some(Uuid::from_u128(20));
        livre.adresse_client_id = Some(Uuid::from_u128(21));
        let etat = appliquer(etat, &livre);
        assert_eq!(etat.client_id, Some(Uuid::from_u128(20)));
        assert_eq!(etat.emplacement_id, None);

        let etat = appliquer(etat, &ev(3, Some(Statut::Recu)));
        assert_eq!(etat.client_id, None, "le client est libéré");
        assert_eq!(etat.lot_id, Some(Uuid::from_u128(11)), "le contenu reste");
    }

    #[test]
    fn le_lavage_est_trace() {
        let laveur = Uuid::from_u128(99);
        let mut e = ev(1, Some(Statut::Propre));
        e.utilisateur_id = Some(laveur);
        let etat = appliquer(EtatFut::default(), &e);
        assert_eq!(etat.dernier_lavage_par, Some(laveur));
        assert_eq!(etat.dernier_lavage_le, Some(e.survenu_le));
    }

    #[test]
    fn statut_depuis_le_ne_bouge_que_sur_changement() {
        let etat = appliquer(EtatFut::default(), &ev(1, Some(Statut::Sale)));
        let t1 = etat.statut_depuis_le;
        // Un événement sans changement de statut ne redémarre pas le compteur.
        let etat = appliquer(etat, &ev(2, None));
        assert_eq!(etat.statut_depuis_le, t1);
    }

    #[test]
    fn reconstruire_egale_application_incrementale() {
        let suite = vec![
            ev(1, Some(Statut::Recu)),
            ev(2, Some(Statut::Sale)),
            ev(3, Some(Statut::Propre)),
        ];
        let incremental = suite.iter().fold(EtatFut::default(), appliquer);
        assert_eq!(reconstruire(&suite), incremental);
    }
}
