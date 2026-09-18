use crate::erreur::ErreurMetier;

/// Contenances proposées par défaut à la création d'une brasserie.
/// Modifiable par locataire : un parc n'est pas homogène et certaines
/// brasseries utilisent du 15 ou du 25 L.
pub const CONTENANCES_PAR_DEFAUT: [i32; 3] = [20, 30, 50];

pub fn verifier(litres: i32, autorisees: &[i32]) -> Result<(), ErreurMetier> {
    if autorisees.contains(&litres) {
        Ok(())
    } else {
        Err(ErreurMetier::ContenanceNonAutorisee { litres })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuse_une_contenance_hors_liste() {
        assert!(verifier(20, &CONTENANCES_PAR_DEFAUT).is_ok());
        assert!(verifier(25, &CONTENANCES_PAR_DEFAUT).is_err());
        assert!(verifier(25, &[15, 25, 50]).is_ok());
    }
}
