//! Recherche globale : fûts (code QR), clients (société, contact) et lots
//! (numéro). Insensible à la casse, limitée à quelques lignes par catégorie.
//! Toujours appelée sur la connexion d'une `TransactionLocataire` : la RLS
//! borne les résultats au locataire.

use sqlx::PgConnection;
use uuid::Uuid;

use crate::Resultat;

/// Nombre maximal de résultats par catégorie.
pub const LIMITE: i64 = 5;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ResultatFut {
    pub id: Uuid,
    pub code_qr: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ResultatClient {
    pub id: Uuid,
    pub societe: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ResultatLot {
    pub id: Uuid,
    pub numero: String,
}

#[derive(Debug, Clone, Default)]
pub struct Resultats {
    pub futs: Vec<ResultatFut>,
    pub clients: Vec<ResultatClient>,
    pub lots: Vec<ResultatLot>,
    /// Un code QR, une société ou un numéro de lot égal exactement à la saisie.
    pub exacts: usize,
}

impl Resultats {
    pub fn total(&self) -> usize {
        self.futs.len() + self.clients.len() + self.lots.len()
    }
}

/// Neutralise les jokers de `ilike` pour que la saisie soit prise au pied
/// de la lettre.
fn motif(saisie: &str) -> String {
    let mut s = String::from("%");
    for c in saisie.chars() {
        if matches!(c, '%' | '_' | '\\') {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('%');
    s
}

pub async fn chercher(ex: &mut PgConnection, saisie: &str) -> Resultat<Resultats> {
    let saisie = saisie.trim();
    let m = motif(saisie);
    let futs = sqlx::query_as::<_, ResultatFut>(
        "select id, code_qr from fut where code_qr ilike $1
          order by (lower(code_qr) = lower($2)) desc, code_qr limit $3",
    )
    .bind(&m)
    .bind(saisie)
    .bind(LIMITE)
    .fetch_all(&mut *ex)
    .await?;
    let clients = sqlx::query_as::<_, ResultatClient>(
        "select id, societe from client
          where societe ilike $1 or contact_nom ilike $1
          order by (lower(societe) = lower($2)) desc, archive, societe limit $3",
    )
    .bind(&m)
    .bind(saisie)
    .bind(LIMITE)
    .fetch_all(&mut *ex)
    .await?;
    let lots = sqlx::query_as::<_, ResultatLot>(
        "select id, numero from lot where numero ilike $1
          order by (lower(numero) = lower($2)) desc, numero desc limit $3",
    )
    .bind(&m)
    .bind(saisie)
    .bind(LIMITE)
    .fetch_all(&mut *ex)
    .await?;
    let bas = saisie.to_lowercase();
    let exacts = futs
        .iter()
        .filter(|f| f.code_qr.to_lowercase() == bas)
        .count()
        + clients
            .iter()
            .filter(|c| c.societe.to_lowercase() == bas)
            .count()
        + lots
            .iter()
            .filter(|l| l.numero.to_lowercase() == bas)
            .count();
    Ok(Resultats {
        futs,
        clients,
        lots,
        exacts,
    })
}
