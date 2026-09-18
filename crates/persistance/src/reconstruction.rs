//! Rejeu du flux d'événements.
//!
//! Corrige un défaut de projection sans perte de donnée, et sert de test de
//! cohérence : rejouer l'histoire doit redonner exactement l'état courant.

use domaine::evenement::EvenementApplique;
use domaine::projection::reconstruire;
use domaine::{EtatFut, Origine, Statut, TypeEvenement};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::Resultat;

#[derive(Debug, Default)]
pub struct Rapport {
    pub futs_examines: i64,
    pub futs_diverges: Vec<Divergence>,
    pub futs_corriges: i64,
}

#[derive(Debug)]
pub struct Divergence {
    pub fut_id: Uuid,
    pub code_qr: String,
    pub details: Vec<String>,
}

async fn evenements_du_fut(
    conn: &mut PgConnection,
    fut_id: Uuid,
) -> Resultat<Vec<EvenementApplique>> {
    Ok(sqlx::query_as!(
        EvenementApplique,
        r#"select id, sequence,
                  type_evenement as "type_evenement: TypeEvenement",
                  statut_apres as "statut_apres: Statut",
                  emplacement_id, client_id, adresse_client_id,
                  type_biere_id, lot_id, cuve_id, utilisateur_id, survenu_le
             from evenement_fut where fut_id = $1 order by sequence"#,
        fut_id
    )
    .fetch_all(&mut *conn)
    .await?)
}

fn comparer(attendu: &EtatFut, courant: &EtatFut) -> Vec<String> {
    let mut d = vec![];
    macro_rules! cmp {
        ($champ:ident) => {
            if attendu.$champ != courant.$champ {
                d.push(format!(
                    "{} : projeté {:?}, rejoué {:?}",
                    stringify!($champ),
                    courant.$champ,
                    attendu.$champ
                ));
            }
        };
    }
    cmp!(statut);
    cmp!(emplacement_id);
    cmp!(client_id);
    cmp!(adresse_client_id);
    cmp!(type_biere_id);
    cmp!(lot_id);
    cmp!(cuve_id);
    cmp!(date_remplissage);
    cmp!(statut_depuis_le);
    cmp!(dernier_lavage_le);
    cmp!(dernier_lavage_par);
    cmp!(sequence_courante);
    d
}

/// `ecrire = false` : vérifie seulement, ne touche à rien. C'est ce que fait
/// l'intégration continue, et un écart fait échouer le travail.
pub async fn executer(conn: &mut PgConnection, ecrire: bool) -> Resultat<Rapport> {
    let futs = sqlx::query!(
        r#"select f.id, f.code_qr, f.statut as "statut: Statut", f.emplacement_id,
                  f.client_id, f.adresse_client_id, f.type_biere_id, f.lot_id, f.cuve_id,
                  f.date_remplissage, f.statut_depuis_le, f.dernier_lavage_le,
                  f.dernier_lavage_par, f.dernier_evenement_id, f.dernier_evenement_le,
                  f.sequence_courante
             from fut f order by f.code_qr"#
    )
    .fetch_all(&mut *conn)
    .await?;

    let mut rapport = Rapport::default();
    for f in futs {
        rapport.futs_examines += 1;
        let courant = EtatFut {
            statut: Some(f.statut),
            emplacement_id: f.emplacement_id,
            client_id: f.client_id,
            adresse_client_id: f.adresse_client_id,
            type_biere_id: f.type_biere_id,
            lot_id: f.lot_id,
            cuve_id: f.cuve_id,
            date_remplissage: f.date_remplissage,
            statut_depuis_le: Some(f.statut_depuis_le),
            dernier_lavage_le: f.dernier_lavage_le,
            dernier_lavage_par: f.dernier_lavage_par,
            dernier_evenement_id: f.dernier_evenement_id,
            dernier_evenement_le: f.dernier_evenement_le,
            sequence_courante: f.sequence_courante,
        };

        let evenements = evenements_du_fut(&mut *conn, f.id).await?;
        if evenements.is_empty() {
            continue;
        }
        let attendu = reconstruire(&evenements);

        let details = comparer(&attendu, &courant);
        if !details.is_empty() {
            rapport.futs_diverges.push(Divergence {
                fut_id: f.id,
                code_qr: f.code_qr.clone(),
                details,
            });
            if ecrire {
                crate::evenement::ecrire_projection(&mut *conn, f.id, &attendu, None).await?;
                rapport.futs_corriges += 1;
            }
        }
    }
    Ok(rapport)
}

/// Dernier événement utile au diagnostic.
pub async fn compter_evenements(conn: &mut PgConnection) -> Resultat<i64> {
    let _ = Origine::Systeme;
    Ok(sqlx::query_scalar!("select count(*) from evenement_fut")
        .fetch_one(&mut *conn)
        .await?
        .unwrap_or(0))
}
