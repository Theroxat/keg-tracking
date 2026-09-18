//! Écriture du flux d'événements et mise à jour de la projection.
//!
//! Toute modification d'un fût passe par ici, et par une seule transaction :
//! verrou sur la ligne, vérification de la transition, insertion de
//! l'événement, recalcul de la projection. Aucun autre chemin n'écrit sur
//! `fut`.

use chrono::{DateTime, Utc};
use domaine::evenement::EvenementApplique;
use domaine::projection::appliquer;
use domaine::{ContexteTransition, EtatFut, Origine, Role, Statut, TypeEvenement};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{Erreur, Resultat, TransactionLocataire};

#[derive(Debug, Clone)]
pub struct DemandeEvenement {
    pub fut_id: Uuid,
    /// `None` = l'événement ne change pas le statut (déplacement, correction
    /// de données, commentaire).
    pub statut_cible: Option<Statut>,
    pub ctx: ContexteTransition,
    pub origine: Origine,
    /// Force le type d'événement : mise en service, correction, import.
    /// Sinon il est déduit (transition, transition forcée, modification).
    pub type_impose: Option<TypeEvenement>,
    pub commentaire: String,
    pub document_id: Option<Uuid>,
    pub survenu_le: Option<DateTime<Utc>>,
}

impl DemandeEvenement {
    pub fn nouvelle(fut_id: Uuid, origine: Origine) -> Self {
        DemandeEvenement {
            fut_id,
            statut_cible: None,
            ctx: ContexteTransition::default(),
            origine,
            type_impose: None,
            commentaire: String::new(),
            document_id: None,
            survenu_le: None,
        }
    }
}

/// État nécessaire pour décider et pour projeter, lu sous verrou.
pub struct EtatVerrouille {
    pub statut: Statut,
    pub etat: EtatFut,
}

/// Lit la ligne du fût **avec `for update`** : deux opérateurs qui scannent
/// le même fût en même temps sont sérialisés, et le second voit l'état réel
/// produit par le premier.
pub async fn verrouiller(conn: &mut PgConnection, fut_id: Uuid) -> Resultat<EtatVerrouille> {
    let r = sqlx::query!(
        r#"select statut as "statut: Statut", emplacement_id, client_id, adresse_client_id,
                  type_biere_id, lot_id, cuve_id, date_remplissage, statut_depuis_le,
                  dernier_lavage_le, dernier_lavage_par, dernier_evenement_id,
                  dernier_evenement_le, sequence_courante
             from fut where id = $1 for update"#,
        fut_id
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Erreur::Introuvable)?;

    Ok(EtatVerrouille {
        statut: r.statut,
        etat: EtatFut {
            statut: Some(r.statut),
            emplacement_id: r.emplacement_id,
            client_id: r.client_id,
            adresse_client_id: r.adresse_client_id,
            type_biere_id: r.type_biere_id,
            lot_id: r.lot_id,
            cuve_id: r.cuve_id,
            date_remplissage: r.date_remplissage,
            statut_depuis_le: r.statut_depuis_le.into(),
            dernier_lavage_le: r.dernier_lavage_le,
            dernier_lavage_par: r.dernier_lavage_par,
            dernier_evenement_id: r.dernier_evenement_id,
            dernier_evenement_le: r.dernier_evenement_le,
            sequence_courante: r.sequence_courante,
        },
    })
}

/// Vérifie la transition demandée sans rien écrire.
///
/// C'est ce que la capture d'un document appelle pour refuser un fût
/// **pendant que l'opérateur l'a en main**, et pas dix minutes plus tard.
pub fn verifier_demande(
    statut_actuel: Statut,
    d: &DemandeEvenement,
    role: Role,
) -> Result<(), domaine::ErreurMetier> {
    match d.statut_cible {
        Some(cible) => domaine::statut::verifier(statut_actuel, cible, &d.ctx, role),
        None => Ok(()),
    }
}

fn type_deduit(d: &DemandeEvenement) -> TypeEvenement {
    if let Some(t) = d.type_impose {
        return t;
    }
    match (d.statut_cible, d.ctx.forcee) {
        (Some(_), true) => TypeEvenement::TransitionForcee,
        (Some(_), false) => TypeEvenement::Transition,
        (None, _) => TypeEvenement::ModificationDonnees,
    }
}

/// Cohérences que la base ne peut pas exprimer seule.
async fn verifier_coherences(conn: &mut PgConnection, d: &DemandeEvenement) -> Resultat<()> {
    if let (Some(lot_id), Some(biere_id)) = (d.ctx.lot_id, d.ctx.type_biere_id) {
        let ok = sqlx::query_scalar!(
            "select exists(select 1 from lot where id = $1 and type_biere_id = $2)",
            lot_id,
            biere_id
        )
        .fetch_one(&mut *conn)
        .await?
        .unwrap_or(false);
        if !ok {
            return Err(domaine::ErreurMetier::LotIncoherentAvecBiere.into());
        }
    }
    if let Some(client_id) = d.ctx.client_id {
        let archive = sqlx::query_scalar!("select archive from client where id = $1", client_id)
            .fetch_optional(&mut *conn)
            .await?;
        match archive {
            None => return Err(Erreur::Introuvable),
            Some(true) => return Err(domaine::ErreurMetier::ClientArchive.into()),
            Some(false) => {}
        }
        if let Some(adresse_id) = d.ctx.adresse_client_id {
            if !crate::client::adresse_appartient(&mut *conn, client_id, adresse_id).await? {
                return Err(domaine::ErreurMetier::AdresseEtrangereAuClient.into());
            }
        }
    }
    Ok(())
}

/// Chemin normal : vérifie puis écrit. Renvoie l'identifiant de l'événement.
pub async fn enregistrer(
    tx: &mut TransactionLocataire,
    role: Role,
    d: &DemandeEvenement,
) -> Resultat<Uuid> {
    let utilisateur_id = tx.utilisateur_id();
    let conn = tx.conn();
    let verrou = verrouiller(conn, d.fut_id).await?;
    verifier_demande(verrou.statut, d, role)?;
    verifier_coherences(conn, d).await?;
    ecrire(conn, utilisateur_id, Some(verrou.statut), verrou.etat, d).await
}

/// Écrit l'événement et recalcule la projection, **sans** vérifier la
/// transition. Réservé à la mise en service et à l'import de reprise, où il
/// n'existe pas d'état antérieur à respecter.
pub async fn ecrire(
    conn: &mut PgConnection,
    utilisateur_id: Option<Uuid>,
    statut_avant: Option<Statut>,
    etat_courant: EtatFut,
    d: &DemandeEvenement,
) -> Resultat<Uuid> {
    let id = Uuid::now_v7();
    let sequence = etat_courant.sequence_courante + 1;
    let survenu_le = d.survenu_le.unwrap_or_else(Utc::now);
    let type_evenement = type_deduit(d);
    let justification = d.ctx.justification.clone().unwrap_or_default();

    sqlx::query!(
        r#"insert into evenement_fut
             (id, tenant_id, fut_id, sequence, type_evenement, origine,
              statut_avant, statut_apres, emplacement_id, client_id, adresse_client_id,
              type_biere_id, lot_id, cuve_id, commentaire, justification,
              document_id, utilisateur_id, survenu_le)
           values ($1, locataire_courant(), $2, $3, $4, $5, $6, $7, $8, $9, $10,
                   $11, $12, $13, $14, $15, $16, $17, $18)"#,
        id,
        d.fut_id,
        sequence,
        type_evenement as TypeEvenement,
        d.origine as Origine,
        statut_avant as Option<Statut>,
        d.statut_cible as Option<Statut>,
        d.ctx.emplacement_id,
        d.ctx.client_id,
        d.ctx.adresse_client_id,
        d.ctx.type_biere_id,
        d.ctx.lot_id,
        d.ctx.cuve_id,
        d.commentaire.trim(),
        justification.trim(),
        d.document_id,
        utilisateur_id,
        survenu_le
    )
    .execute(&mut *conn)
    .await
    .map_err(Erreur::depuis_contrainte)?;

    let applique = EvenementApplique {
        id,
        sequence,
        type_evenement,
        statut_apres: d.statut_cible,
        emplacement_id: d.ctx.emplacement_id,
        client_id: d.ctx.client_id,
        adresse_client_id: d.ctx.adresse_client_id,
        type_biere_id: d.ctx.type_biere_id,
        lot_id: d.ctx.lot_id,
        cuve_id: d.ctx.cuve_id,
        utilisateur_id,
        survenu_le,
    };
    let nouvel_etat = appliquer(etat_courant, &applique);
    ecrire_projection(&mut *conn, d.fut_id, &nouvel_etat, utilisateur_id).await?;
    Ok(id)
}

/// Écrit la projection sur la ligne du fût. Le seul endroit du programme qui
/// met à jour ces colonnes.
pub async fn ecrire_projection(
    conn: &mut PgConnection,
    fut_id: Uuid,
    e: &EtatFut,
    modifie_par: Option<Uuid>,
) -> Resultat<()> {
    sqlx::query!(
        r#"update fut set
             statut = coalesce($2, statut),
             emplacement_id = $3, client_id = $4, adresse_client_id = $5,
             type_biere_id = $6, lot_id = $7, cuve_id = $8,
             date_remplissage = $9, statut_depuis_le = coalesce($10, statut_depuis_le),
             dernier_lavage_le = $11, dernier_lavage_par = $12,
             dernier_evenement_id = $13, dernier_evenement_le = $14,
             sequence_courante = $15, modifie_le = now(), modifie_par = $16
           where id = $1"#,
        fut_id,
        e.statut as Option<Statut>,
        e.emplacement_id,
        e.client_id,
        e.adresse_client_id,
        e.type_biere_id,
        e.lot_id,
        e.cuve_id,
        e.date_remplissage,
        e.statut_depuis_le,
        e.dernier_lavage_le,
        e.dernier_lavage_par,
        e.dernier_evenement_id,
        e.dernier_evenement_le,
        e.sequence_courante,
        modifie_par
    )
    .execute(&mut *conn)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct EntreeChronologie {
    pub id: Uuid,
    pub sequence: i64,
    pub type_evenement: TypeEvenement,
    pub origine: Origine,
    pub statut_avant: Option<Statut>,
    pub statut_apres: Option<Statut>,
    pub emplacement: Option<String>,
    pub client: Option<String>,
    pub biere: Option<String>,
    pub lot: Option<String>,
    pub cuve: Option<String>,
    pub commentaire: String,
    pub justification: String,
    pub par: Option<String>,
    pub document_reference: Option<String>,
    pub survenu_le: DateTime<Utc>,
}

pub async fn chronologie(
    conn: &mut PgConnection,
    fut_id: Uuid,
) -> Resultat<Vec<EntreeChronologie>> {
    Ok(sqlx::query_as!(
        EntreeChronologie,
        r#"select e.id, e.sequence,
                  e.type_evenement as "type_evenement: TypeEvenement",
                  e.origine as "origine: Origine",
                  e.statut_avant as "statut_avant: Statut",
                  e.statut_apres as "statut_apres: Statut",
                  em.nom as "emplacement?", c.societe as "client?",
                  b.nom as "biere?", l.numero as "lot?", cv.nom as "cuve?",
                  e.commentaire, e.justification,
                  nullif(trim(coalesce(u.prenom,'') || ' ' || coalesce(u.nom,'')), '') as "par?",
                  d.reference as "document_reference?",
                  e.survenu_le
             from evenement_fut e
             left join emplacement em on em.id = e.emplacement_id
             left join client c       on c.id  = e.client_id
             left join type_biere b   on b.id  = e.type_biere_id
             left join lot l          on l.id  = e.lot_id
             left join cuve cv        on cv.id = e.cuve_id
             left join utilisateur u  on u.id  = e.utilisateur_id
             left join document_mouvement d on d.id = e.document_id
            where e.fut_id = $1
            order by e.sequence desc"#,
        fut_id
    )
    .fetch_all(&mut *conn)
    .await?)
}

#[derive(Debug, Clone)]
pub struct ActiviteRecente {
    pub fut_id: Uuid,
    pub code_qr: String,
    pub statut_apres: Option<Statut>,
    pub type_evenement: TypeEvenement,
    pub par: Option<String>,
    pub survenu_le: DateTime<Utc>,
}

pub async fn activite_recente(
    conn: &mut PgConnection,
    limite: i64,
) -> Resultat<Vec<ActiviteRecente>> {
    Ok(sqlx::query_as!(
        ActiviteRecente,
        r#"select e.fut_id, f.code_qr,
                  e.statut_apres as "statut_apres: Statut",
                  e.type_evenement as "type_evenement: TypeEvenement",
                  nullif(trim(coalesce(u.prenom,'') || ' ' || coalesce(u.nom,'')), '') as "par?",
                  e.survenu_le
             from evenement_fut e
             join fut f on f.id = e.fut_id
             left join utilisateur u on u.id = e.utilisateur_id
            order by e.survenu_le desc limit $1"#,
        limite
    )
    .fetch_all(&mut *conn)
    .await?)
}
