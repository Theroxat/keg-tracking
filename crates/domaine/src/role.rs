use serde::{Deserialize, Serialize};

/// Trois rôles fixes. Pas de permissions granulaires : pour une brasserie de
/// cinq personnes, onze booléens par utilisateur sont de la complexité que
/// personne n'utilise.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(type_name = "role_utilisateur", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Consultation et export. Aucune écriture.
    Lecture,
    /// Scan, documents de mouvement, modification de fûts.
    Production,
    /// Tout dans sa brasserie.
    Administrateur,
}

impl Role {
    pub const TOUS: [Role; 3] = [Role::Lecture, Role::Production, Role::Administrateur];

    pub const fn code(self) -> &'static str {
        match self {
            Role::Lecture => "lecture",
            Role::Production => "production",
            Role::Administrateur => "administrateur",
        }
    }

    pub const fn cle_i18n(self) -> &'static str {
        match self {
            Role::Lecture => "role.lecture",
            Role::Production => "role.production",
            Role::Administrateur => "role.administrateur",
        }
    }

    pub fn depuis_code(code: &str) -> Option<Role> {
        Role::TOUS.into_iter().find(|r| r.code() == code)
    }

    /// Ce rôle satisfait-il l'exigence d'une route ?
    /// L'ordre de `Role` est délibérément croissant en pouvoir.
    pub fn permet(self, requis: Role) -> bool {
        self >= requis
    }

    /// Le rôle peut-il écrire quoi que ce soit ?
    pub const fn peut_ecrire(self) -> bool {
        !matches!(self, Role::Lecture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchie() {
        assert!(Role::Administrateur.permet(Role::Lecture));
        assert!(Role::Administrateur.permet(Role::Production));
        assert!(Role::Production.permet(Role::Lecture));
        assert!(!Role::Production.permet(Role::Administrateur));
        assert!(!Role::Lecture.permet(Role::Production));
        assert!(Role::Lecture.permet(Role::Lecture));
    }

    #[test]
    fn lecture_n_ecrit_pas() {
        assert!(!Role::Lecture.peut_ecrire());
        assert!(Role::Production.peut_ecrire());
        assert!(Role::Administrateur.peut_ecrire());
    }
}
