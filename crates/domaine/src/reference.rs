use serde::{Deserialize, Serialize};

/// Type d'un document de mouvement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "type_document", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TypeDocument {
    Livraison,
    Retour,
    OperationInterne,
}

impl TypeDocument {
    pub const TOUS: [TypeDocument; 3] = [
        TypeDocument::Livraison,
        TypeDocument::Retour,
        TypeDocument::OperationInterne,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            TypeDocument::Livraison => "livraison",
            TypeDocument::Retour => "retour",
            TypeDocument::OperationInterne => "operation_interne",
        }
    }

    /// Préfixe de la référence : LIV-2026-0001, RET-…, INT-…
    pub const fn prefixe(self) -> &'static str {
        match self {
            TypeDocument::Livraison => "LIV",
            TypeDocument::Retour => "RET",
            TypeDocument::OperationInterne => "INT",
        }
    }

    pub const fn cle_i18n(self) -> &'static str {
        match self {
            TypeDocument::Livraison => "document.type.livraison",
            TypeDocument::Retour => "document.type.retour",
            TypeDocument::OperationInterne => "document.type.operation_interne",
        }
    }

    /// Une livraison exige un client et une adresse sur le document lui-même.
    pub const fn exige_client(self) -> bool {
        matches!(self, TypeDocument::Livraison)
    }

    pub fn depuis_code(code: &str) -> Option<TypeDocument> {
        TypeDocument::TOUS.into_iter().find(|t| t.code() == code)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "etat_document", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EtatDocument {
    Brouillon,
    Valide,
    Abandonne,
}

impl EtatDocument {
    pub const fn code(self) -> &'static str {
        match self {
            EtatDocument::Brouillon => "brouillon",
            EtatDocument::Valide => "valide",
            EtatDocument::Abandonne => "abandonne",
        }
    }
    pub const fn cle_i18n(self) -> &'static str {
        match self {
            EtatDocument::Brouillon => "document.etat.brouillon",
            EtatDocument::Valide => "document.etat.valide",
            EtatDocument::Abandonne => "document.etat.abandonne",
        }
    }
    pub const fn modifiable(self) -> bool {
        matches!(self, EtatDocument::Brouillon)
    }
}

/// `LIV-2026-0001`
pub fn formater(type_doc: TypeDocument, annee: i32, numero: i64) -> String {
    format!("{}-{}-{:04}", type_doc.prefixe(), annee, numero)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_de_reference() {
        assert_eq!(formater(TypeDocument::Livraison, 2026, 1), "LIV-2026-0001");
        assert_eq!(
            formater(TypeDocument::OperationInterne, 2026, 1234),
            "INT-2026-1234"
        );
    }
}
