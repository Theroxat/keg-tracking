use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::erreur::ErreurMetier;
use crate::role::Role;

/// Statut d'un fût dans son cycle de vie.
///
/// Type fermé : aucune comparaison de chaîne, aucun accent. Les libellés
/// affichés vivent dans le catalogue de traduction, jamais ici.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(type_name = "statut_fut", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Statut {
    Recu,
    Sale,
    Propre,
    Rempli,
    Livre,
    HorsService,
    Perdu,
}

impl Statut {
    pub const TOUS: [Statut; 7] = [
        Statut::Recu,
        Statut::Sale,
        Statut::Propre,
        Statut::Rempli,
        Statut::Livre,
        Statut::HorsService,
        Statut::Perdu,
    ];

    /// Statuts d'un fût encore en service (exclut les sorties de parc).
    pub const EN_SERVICE: [Statut; 5] = [
        Statut::Recu,
        Statut::Sale,
        Statut::Propre,
        Statut::Rempli,
        Statut::Livre,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Statut::Recu => "recu",
            Statut::Sale => "sale",
            Statut::Propre => "propre",
            Statut::Rempli => "rempli",
            Statut::Livre => "livre",
            Statut::HorsService => "hors_service",
            Statut::Perdu => "perdu",
        }
    }

    pub const fn cle_i18n(self) -> &'static str {
        match self {
            Statut::Recu => "statut.recu",
            Statut::Sale => "statut.sale",
            Statut::Propre => "statut.propre",
            Statut::Rempli => "statut.rempli",
            Statut::Livre => "statut.livre",
            Statut::HorsService => "statut.hors_service",
            Statut::Perdu => "statut.perdu",
        }
    }

    pub fn depuis_code(code: &str) -> Option<Statut> {
        Statut::TOUS.into_iter().find(|s| s.code() == code)
    }

    /// Le fût est-il physiquement détenu par un client ?
    pub const fn chez_le_client(self) -> bool {
        matches!(self, Statut::Livre)
    }

    /// Le fût compte-t-il encore dans le parc exploitable ?
    pub const fn dans_le_parc(self) -> bool {
        !matches!(self, Statut::HorsService | Statut::Perdu)
    }

    /// Table des transitions autorisées. Toute évolution du cycle de vie
    /// passe par cette fonction et par elle seule.
    pub const fn transitions_valides(self) -> &'static [Statut] {
        use Statut::*;
        match self {
            Recu => &[Sale, Propre, HorsService, Perdu],
            Sale => &[Propre, HorsService, Perdu],
            Propre => &[Rempli, Sale, HorsService, Perdu],
            Rempli => &[Livre, Sale, HorsService, Perdu],
            Livre => &[Recu, Sale, HorsService, Perdu],
            HorsService => &[Sale, Propre, Perdu],
            Perdu => &[Recu, Sale, HorsService],
        }
    }

    pub fn peut_aller_vers(self, cible: Statut) -> bool {
        self.transitions_valides().contains(&cible)
    }
}

/// Contexte fourni à une transition : les valeurs que l'opération veut poser
/// sur le fût, plus qui la demande.
#[derive(Debug, Clone, Default)]
pub struct ContexteTransition {
    pub type_biere_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub cuve_id: Option<Uuid>,
    pub client_id: Option<Uuid>,
    pub adresse_client_id: Option<Uuid>,
    pub emplacement_id: Option<Uuid>,
    pub justification: Option<String>,
    pub forcee: bool,
}

fn renseigne(v: &Option<String>) -> bool {
    v.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false)
}

/// Vérifie qu'une transition est légale et que ses préconditions sont
/// réunies. C'est le seul point d'entrée : aucun appelant ne doit
/// reconstruire cette logique.
pub fn verifier(
    avant: Statut,
    apres: Statut,
    ctx: &ContexteTransition,
    role: Role,
) -> Result<(), ErreurMetier> {
    if avant == apres {
        return Err(ErreurMetier::DejaDansCeStatut { statut: apres });
    }

    if ctx.forcee {
        if role != Role::Administrateur {
            return Err(ErreurMetier::ForcageReserveAdministrateur);
        }
        if !renseigne(&ctx.justification) {
            return Err(ErreurMetier::JustificationRequise);
        }
    } else if !avant.peut_aller_vers(apres) {
        return Err(ErreurMetier::TransitionInterdite { avant, apres });
    }

    // Les préconditions tiennent même sur une transition forcée : on ne peut
    // pas livrer sans client, quel que soit le rôle.
    verifier_preconditions(apres, ctx)
}

/// Préconditions du statut cible, indépendamment du statut de départ.
pub fn verifier_preconditions(apres: Statut, ctx: &ContexteTransition) -> Result<(), ErreurMetier> {
    match apres {
        Statut::Rempli => {
            if ctx.type_biere_id.is_none() {
                return Err(ErreurMetier::PreconditionManquante {
                    statut: apres,
                    champ: "type_biere",
                });
            }
            if ctx.lot_id.is_none() {
                return Err(ErreurMetier::PreconditionManquante {
                    statut: apres,
                    champ: "lot",
                });
            }
            if ctx.cuve_id.is_none() {
                return Err(ErreurMetier::PreconditionManquante {
                    statut: apres,
                    champ: "cuve",
                });
            }
        }
        Statut::Livre => {
            if ctx.client_id.is_none() {
                return Err(ErreurMetier::PreconditionManquante {
                    statut: apres,
                    champ: "client",
                });
            }
            if ctx.adresse_client_id.is_none() {
                return Err(ErreurMetier::PreconditionManquante {
                    statut: apres,
                    champ: "adresse",
                });
            }
        }
        Statut::Perdu | Statut::HorsService if !renseigne(&ctx.justification) => {
            return Err(ErreurMetier::JustificationRequise);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_complet() -> ContexteTransition {
        ContexteTransition {
            type_biere_id: Some(Uuid::nil()),
            lot_id: Some(Uuid::nil()),
            cuve_id: Some(Uuid::nil()),
            client_id: Some(Uuid::nil()),
            adresse_client_id: Some(Uuid::nil()),
            emplacement_id: Some(Uuid::nil()),
            justification: Some("motif".into()),
            forcee: false,
        }
    }

    /// Les 49 couples, comparés à la table de référence du document
    /// d'architecture. Ce test est la définition exécutable du cycle de vie.
    #[test]
    fn matrice_complete_des_transitions() {
        use Statut::*;
        let attendu: &[(Statut, &[Statut])] = &[
            (Recu, &[Sale, Propre, HorsService, Perdu]),
            (Sale, &[Propre, HorsService, Perdu]),
            (Propre, &[Rempli, Sale, HorsService, Perdu]),
            (Rempli, &[Livre, Sale, HorsService, Perdu]),
            (Livre, &[Recu, Sale, HorsService, Perdu]),
            (HorsService, &[Sale, Propre, Perdu]),
            (Perdu, &[Recu, Sale, HorsService]),
        ];
        for (depuis, autorises) in attendu {
            for vers in Statut::TOUS {
                let permis = autorises.contains(&vers);
                assert_eq!(
                    depuis.peut_aller_vers(vers),
                    permis,
                    "{:?} -> {:?}",
                    depuis,
                    vers
                );
            }
        }
    }

    #[test]
    fn aucun_statut_n_est_un_puits() {
        for s in Statut::TOUS {
            assert!(
                !s.transitions_valides().is_empty(),
                "{:?} n'a aucune sortie",
                s
            );
        }
    }

    #[test]
    fn transition_interdite_est_refusee() {
        let e = verifier(
            Statut::Sale,
            Statut::Livre,
            &ctx_complet(),
            Role::Administrateur,
        )
        .unwrap_err();
        assert!(matches!(e, ErreurMetier::TransitionInterdite { .. }));
    }

    #[test]
    fn remplissage_exige_biere_lot_cuve() {
        let mut ctx = ctx_complet();
        ctx.lot_id = None;
        let e = verifier(Statut::Propre, Statut::Rempli, &ctx, Role::Production).unwrap_err();
        assert!(matches!(
            e,
            ErreurMetier::PreconditionManquante { champ: "lot", .. }
        ));
    }

    #[test]
    fn livraison_exige_client_et_adresse() {
        let mut ctx = ctx_complet();
        ctx.adresse_client_id = None;
        let e = verifier(Statut::Rempli, Statut::Livre, &ctx, Role::Production).unwrap_err();
        assert!(matches!(
            e,
            ErreurMetier::PreconditionManquante {
                champ: "adresse",
                ..
            }
        ));
    }

    #[test]
    fn perte_exige_une_justification() {
        let mut ctx = ctx_complet();
        ctx.justification = Some("   ".into());
        let e = verifier(Statut::Livre, Statut::Perdu, &ctx, Role::Production).unwrap_err();
        assert!(matches!(e, ErreurMetier::JustificationRequise));
    }

    #[test]
    fn forcage_reserve_a_l_administrateur() {
        let mut ctx = ctx_complet();
        ctx.forcee = true;
        let e = verifier(Statut::Propre, Statut::Livre, &ctx, Role::Production).unwrap_err();
        assert!(matches!(e, ErreurMetier::ForcageReserveAdministrateur));
        // Le même forçage passe pour un administrateur.
        verifier(Statut::Propre, Statut::Livre, &ctx, Role::Administrateur).unwrap();
    }

    #[test]
    fn forcage_ne_dispense_pas_des_preconditions() {
        let mut ctx = ctx_complet();
        ctx.forcee = true;
        ctx.client_id = None;
        let e = verifier(Statut::Propre, Statut::Livre, &ctx, Role::Administrateur).unwrap_err();
        assert!(matches!(
            e,
            ErreurMetier::PreconditionManquante {
                champ: "client",
                ..
            }
        ));
    }

    #[test]
    fn meme_statut_est_refuse() {
        let e = verifier(
            Statut::Sale,
            Statut::Sale,
            &ctx_complet(),
            Role::Administrateur,
        )
        .unwrap_err();
        assert!(matches!(e, ErreurMetier::DejaDansCeStatut { .. }));
    }

    #[test]
    fn codes_et_aller_retour() {
        for s in Statut::TOUS {
            assert_eq!(Statut::depuis_code(s.code()), Some(s));
        }
        assert_eq!(Statut::depuis_code("Reçu"), None);
    }
}
