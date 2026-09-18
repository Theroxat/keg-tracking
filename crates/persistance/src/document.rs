//! Le document de mouvement.
//!
//! Affecter vingt fûts à un client, ce ne sont pas vingt modifications
//! indépendantes : c'est une livraison. Le brouillon vit côté serveur dès le
//! premier scan — c'est ce qui rend une couche hors-ligne inutile.

use chrono::{DateTime, Utc};
use domaine::{EtatDocument, Origine, Role, Statut, TypeDocument};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::evenement::{self, DemandeEvenement};
use crate::{Erreur, Resultat, TransactionLocataire};

#[derive(Debug, Clone, Default)]
pub struct Configuration {
    pub statut_cible: Option<Statut>,
    pub type_biere_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub cuve_id: Option<Uuid>,
    pub emplacement_id: Option<Uuid>,
    pub client_id: Option<Uuid>,
    pub adresse_client_id: Option<Uuid>,
    /// Saisi **avant** la capture : l'opérateur n'a plus rien à taper ensuite.
    pub commentaire: String,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub id: Uuid,
    pub type_document: TypeDocument,
    pub reference: String,
    pub etat: EtatDocument,
    pub statut_cible: Option<Statut>,
    pub type_biere_id: Option<Uuid>,
    pub biere: Option<String>,
    pub lot_id: Option<Uuid>,
    pub lot: Option<String>,
    pub cuve_id: Option<Uuid>,
    pub cuve: Option<String>,
    pub emplacement_id: Option<Uuid>,
    pub emplacement: Option<String>,
    pub client_id: Option<Uuid>,
    pub client: Option<String>,
    pub adresse_client_id: Option<Uuid>,
    pub adresse: Option<String>,
    pub commentaire: String,
    pub cree_par: Option<Uuid>,
    pub cree_par_nom: Option<String>,
    pub cree_le: DateTime<Utc>,
    pub modifie_le: DateTime<Utc>,
    pub valide_par_nom: Option<String>,
    pub valide_le: Option<DateTime<Utc>>,
    pub lignes: i64,
}

impl Document {
    pub fn configuration(&self) -> Configuration {
        Configuration {
            statut_cible: self.statut_cible,
            type_biere_id: self.type_biere_id,
            lot_id: self.lot_id,
            cuve_id: self.cuve_id,
            emplacement_id: self.emplacement_id,
            client_id: self.client_id,
            adresse_client_id: self.adresse_client_id,
            commentaire: self.commentaire.clone(),
        }
    }
}

pub async fn lire(conn: &mut PgConnection, id: Uuid) -> Resultat<Document> {
    sqlx::query_as!(
        Document,
        r#"select d.id, d.type_document as "type_document: TypeDocument", d.reference,
                  d.etat as "etat: EtatDocument", d.statut_cible as "statut_cible: Statut",
                  d.type_biere_id, b.nom as "biere?", d.lot_id, l.numero as "lot?",
                  d.cuve_id, cv.nom as "cuve?", d.emplacement_id, em.nom as "emplacement?",
                  d.client_id, c.societe as "client?", d.adresse_client_id,
                  nullif(trim(coalesce(a.libelle,'') || ' - ' || coalesce(a.rue,'') || ' '
                         || coalesce(a.code_postal,'') || ' ' || coalesce(a.ville,'')), '- ')
                    as "adresse?",
                  d.commentaire, d.cree_par,
                  nullif(trim(coalesce(uc.prenom,'') || ' ' || coalesce(uc.nom,'')), '')
                    as "cree_par_nom?",
                  d.cree_le, d.modifie_le,
                  nullif(trim(coalesce(uv.prenom,'') || ' ' || coalesce(uv.nom,'')), '')
                    as "valide_par_nom?",
                  d.valide_le,
                  (select count(*) from ligne_document ld where ld.document_id = d.id)
                    as "lignes!"
             from document_mouvement d
             left join type_biere b   on b.id  = d.type_biere_id
             left join lot l          on l.id  = d.lot_id
             left join cuve cv        on cv.id = d.cuve_id
             left join emplacement em on em.id = d.emplacement_id
             left join client c       on c.id  = d.client_id
             left join adresse_client a on a.id = d.adresse_client_id
             left join utilisateur uc on uc.id = d.cree_par
             left join utilisateur uv on uv.id = d.valide_par
            where d.id = $1"#,
        id
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Erreur::Introuvable)
}

pub async fn lister(
    conn: &mut PgConnection,
    etat: Option<EtatDocument>,
    limite: i64,
) -> Resultat<Vec<Document>> {
    Ok(sqlx::query_as!(
        Document,
        r#"select d.id, d.type_document as "type_document: TypeDocument", d.reference,
                  d.etat as "etat: EtatDocument", d.statut_cible as "statut_cible: Statut",
                  d.type_biere_id, b.nom as "biere?", d.lot_id, l.numero as "lot?",
                  d.cuve_id, cv.nom as "cuve?", d.emplacement_id, em.nom as "emplacement?",
                  d.client_id, c.societe as "client?", d.adresse_client_id,
                  nullif(trim(coalesce(a.libelle,'') || ' - ' || coalesce(a.rue,'') || ' '
                         || coalesce(a.code_postal,'') || ' ' || coalesce(a.ville,'')), '- ')
                    as "adresse?",
                  d.commentaire, d.cree_par,
                  nullif(trim(coalesce(uc.prenom,'') || ' ' || coalesce(uc.nom,'')), '')
                    as "cree_par_nom?",
                  d.cree_le, d.modifie_le,
                  nullif(trim(coalesce(uv.prenom,'') || ' ' || coalesce(uv.nom,'')), '')
                    as "valide_par_nom?",
                  d.valide_le,
                  (select count(*) from ligne_document ld where ld.document_id = d.id)
                    as "lignes!"
             from document_mouvement d
             left join type_biere b   on b.id  = d.type_biere_id
             left join lot l          on l.id  = d.lot_id
             left join cuve cv        on cv.id = d.cuve_id
             left join emplacement em on em.id = d.emplacement_id
             left join client c       on c.id  = d.client_id
             left join adresse_client a on a.id = d.adresse_client_id
             left join utilisateur uc on uc.id = d.cree_par
             left join utilisateur uv on uv.id = d.valide_par
            where ($1::etat_document is null or d.etat = $1)
            order by d.modifie_le desc limit $2"#,
        etat as Option<EtatDocument>,
        limite
    )
    .fetch_all(&mut *conn)
    .await?)
}

/// Reprise automatique : l'opérateur qui revient sur la page retrouve son
/// brouillon. Rafraîchissement, veille, batterie à plat, changement
/// d'appareil — la capture ne se perd pas.
pub async fn brouillon_en_cours(
    conn: &mut PgConnection,
    utilisateur_id: Uuid,
) -> Resultat<Option<Document>> {
    Ok(sqlx::query_as!(
        Document,
        r#"select d.id, d.type_document as "type_document: TypeDocument", d.reference,
                  d.etat as "etat: EtatDocument", d.statut_cible as "statut_cible: Statut",
                  d.type_biere_id, b.nom as "biere?", d.lot_id, l.numero as "lot?",
                  d.cuve_id, cv.nom as "cuve?", d.emplacement_id, em.nom as "emplacement?",
                  d.client_id, c.societe as "client?", d.adresse_client_id,
                  nullif(trim(coalesce(a.libelle,'') || ' - ' || coalesce(a.rue,'') || ' '
                         || coalesce(a.code_postal,'') || ' ' || coalesce(a.ville,'')), '- ')
                    as "adresse?",
                  d.commentaire, d.cree_par,
                  nullif(trim(coalesce(uc.prenom,'') || ' ' || coalesce(uc.nom,'')), '')
                    as "cree_par_nom?",
                  d.cree_le, d.modifie_le,
                  nullif(trim(coalesce(uv.prenom,'') || ' ' || coalesce(uv.nom,'')), '')
                    as "valide_par_nom?",
                  d.valide_le,
                  (select count(*) from ligne_document ld where ld.document_id = d.id)
                    as "lignes!"
             from document_mouvement d
             left join type_biere b   on b.id  = d.type_biere_id
             left join lot l          on l.id  = d.lot_id
             left join cuve cv        on cv.id = d.cuve_id
             left join emplacement em on em.id = d.emplacement_id
             left join client c       on c.id  = d.client_id
             left join adresse_client a on a.id = d.adresse_client_id
             left join utilisateur uc on uc.id = d.cree_par
             left join utilisateur uv on uv.id = d.valide_par
            where d.etat = 'brouillon' and d.cree_par = $1
            order by d.modifie_le desc limit 1"#,
        utilisateur_id
    )
    .fetch_optional(&mut *conn)
    .await?)
}

pub async fn creer(
    conn: &mut PgConnection,
    type_document: TypeDocument,
    cfg: &Configuration,
    utilisateur_id: Option<Uuid>,
) -> Resultat<Uuid> {
    let annee = Utc::now().format("%Y").to_string();
    let cle = format!("{}-{}", type_document.prefixe(), annee);
    let numero = crate::compteur::prochain(&mut *conn, &cle).await?;
    let reference =
        domaine::reference::formater(type_document, annee.parse().unwrap_or(2026), numero);

    sqlx::query_scalar!(
        r#"insert into document_mouvement
             (tenant_id, type_document, reference, statut_cible, type_biere_id,
              lot_id, cuve_id, emplacement_id, client_id, adresse_client_id,
              commentaire, cree_par)
           values (locataire_courant(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
           returning id"#,
        type_document as TypeDocument,
        reference,
        cfg.statut_cible as Option<Statut>,
        cfg.type_biere_id,
        cfg.lot_id,
        cfg.cuve_id,
        cfg.emplacement_id,
        cfg.client_id,
        cfg.adresse_client_id,
        cfg.commentaire.trim(),
        utilisateur_id
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(Erreur::depuis_contrainte)
}

pub async fn configurer(conn: &mut PgConnection, id: Uuid, cfg: &Configuration) -> Resultat<()> {
    let n = sqlx::query!(
        r#"update document_mouvement
              set statut_cible = $2, type_biere_id = $3, lot_id = $4, cuve_id = $5,
                  emplacement_id = $6, client_id = $7, adresse_client_id = $8,
                  commentaire = $9, modifie_le = now()
            where id = $1 and etat = 'brouillon'"#,
        id,
        cfg.statut_cible as Option<Statut>,
        cfg.type_biere_id,
        cfg.lot_id,
        cfg.cuve_id,
        cfg.emplacement_id,
        cfg.client_id,
        cfg.adresse_client_id,
        cfg.commentaire.trim()
    )
    .execute(&mut *conn)
    .await
    .map_err(Erreur::depuis_contrainte)?;
    if n.rows_affected() == 0 {
        return Err(domaine::ErreurMetier::DocumentNonModifiable.into());
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct LigneCapturee {
    pub id: Uuid,
    pub fut_id: Uuid,
    pub code_qr: String,
    pub contenance_litres: i32,
    pub statut_avant_capture: Statut,
    pub statut_actuel: Statut,
    pub biere: Option<String>,
    pub lot: Option<String>,
    pub client: Option<String>,
    pub capture_le: DateTime<Utc>,
    pub ordre: i32,
}

pub async fn lignes(conn: &mut PgConnection, document_id: Uuid) -> Resultat<Vec<LigneCapturee>> {
    Ok(sqlx::query_as!(
        LigneCapturee,
        r#"select ld.id, ld.fut_id, f.code_qr, f.contenance_litres,
                  ld.statut_avant_capture as "statut_avant_capture: Statut",
                  f.statut as "statut_actuel: Statut",
                  b.nom as "biere?", l.numero as "lot?", c.societe as "client?",
                  ld.capture_le, ld.ordre
             from ligne_document ld
             join fut f on f.id = ld.fut_id
             left join type_biere b on b.id = f.type_biere_id
             left join lot l        on l.id = f.lot_id
             left join client c     on c.id = f.client_id
            where ld.document_id = $1
            order by ld.ordre desc"#,
        document_id
    )
    .fetch_all(&mut *conn)
    .await?)
}

/// Résultat d'une capture. Les quatre issues sont traitées **au scan**, pas
/// à la validation : l'opérateur l'apprend pendant qu'il a le fût en main.
#[derive(Debug)]
pub enum ResultatCapture {
    Ajoute(Box<LigneCapturee>),
    DejaPresent(Box<LigneCapturee>),
    Refuse {
        code_qr: String,
        statut: Statut,
        raison: domaine::ErreurMetier,
    },
}

pub async fn capturer(
    conn: &mut PgConnection,
    doc: &Document,
    code_qr: &str,
    role: Role,
    utilisateur_id: Option<Uuid>,
) -> Resultat<ResultatCapture> {
    if !doc.etat.modifiable() {
        return Err(domaine::ErreurMetier::DocumentNonModifiable.into());
    }
    // Code inconnu : remonte en ErreurMetier::CodeQrInconnu, jamais en création.
    let fut = crate::fut::lire_par_code(&mut *conn, code_qr).await?;

    // Dédoublonnage : l'unicité est en base, mais on répond proprement.
    if let Some(existante) = lignes(&mut *conn, doc.id)
        .await?
        .into_iter()
        .find(|l| l.fut_id == fut.id)
    {
        return Ok(ResultatCapture::DejaPresent(Box::new(existante)));
    }

    // Refus anticipé : la transition est évaluée maintenant.
    let demande = demande_depuis(doc, fut.id);
    if let Err(raison) = evenement::verifier_demande(fut.statut, &demande, role) {
        return Ok(ResultatCapture::Refuse {
            code_qr: fut.code_qr,
            statut: fut.statut,
            raison,
        });
    }

    let ordre = sqlx::query_scalar!(
        "select coalesce(max(ordre), 0) + 1 from ligne_document where document_id = $1",
        doc.id
    )
    .fetch_one(&mut *conn)
    .await?
    .unwrap_or(1);

    let id = sqlx::query_scalar!(
        r#"insert into ligne_document
             (tenant_id, document_id, fut_id, statut_avant_capture, capture_par, ordre)
           values (locataire_courant(), $1, $2, $3, $4, $5) returning id"#,
        doc.id,
        fut.id,
        fut.statut as Statut,
        utilisateur_id,
        ordre
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(Erreur::depuis_contrainte)?;

    sqlx::query!(
        "update document_mouvement set modifie_le = now() where id = $1",
        doc.id
    )
    .execute(&mut *conn)
    .await?;

    Ok(ResultatCapture::Ajoute(Box::new(LigneCapturee {
        id,
        fut_id: fut.id,
        code_qr: fut.code_qr,
        contenance_litres: fut.contenance_litres,
        statut_avant_capture: fut.statut,
        statut_actuel: fut.statut,
        biere: fut.biere,
        lot: fut.lot,
        client: fut.client,
        capture_le: Utc::now(),
        ordre,
    })))
}

fn demande_depuis(doc: &Document, fut_id: Uuid) -> DemandeEvenement {
    let mut d = DemandeEvenement::nouvelle(fut_id, Origine::Document);
    d.statut_cible = doc.statut_cible;
    d.ctx.type_biere_id = doc.type_biere_id;
    d.ctx.lot_id = doc.lot_id;
    d.ctx.cuve_id = doc.cuve_id;
    d.ctx.emplacement_id = doc.emplacement_id;
    d.ctx.client_id = doc.client_id;
    d.ctx.adresse_client_id = doc.adresse_client_id;
    d.commentaire = doc.commentaire.clone();
    d.document_id = Some(doc.id);
    d
}

pub async fn retirer_ligne(
    conn: &mut PgConnection,
    document_id: Uuid,
    ligne_id: Uuid,
) -> Resultat<()> {
    sqlx::query!(
        r#"delete from ligne_document ld using document_mouvement d
            where ld.id = $2 and ld.document_id = $1
              and d.id = ld.document_id and d.etat = 'brouillon'"#,
        document_id,
        ligne_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn vider(conn: &mut PgConnection, document_id: Uuid) -> Resultat<u64> {
    Ok(sqlx::query!(
        r#"delete from ligne_document ld using document_mouvement d
            where ld.document_id = $1 and d.id = ld.document_id and d.etat = 'brouillon'"#,
        document_id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected())
}

/// Vider un brouillon ne l'efface pas, il le clôt : sinon la piste d'audit
/// aurait un trou.
pub async fn abandonner(conn: &mut PgConnection, document_id: Uuid) -> Resultat<()> {
    sqlx::query!(
        r#"update document_mouvement set etat = 'abandonne', modifie_le = now()
            where id = $1 and etat = 'brouillon'"#,
        document_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[derive(Debug)]
pub struct RapportValidation {
    pub reference: String,
    pub futs: usize,
    pub statut_cible: Option<Statut>,
}

/// Validation transactionnelle : **tout ou rien**.
///
/// Les fûts sont verrouillés dans l'ordre de leur identifiant, ce qui évite
/// les interblocages entre deux validations concurrentes. Chaque transition
/// est réévaluée, car l'état a pu changer depuis la capture. Une seule ligne
/// en erreur, et rien n'est écrit : l'appelant laisse tomber la transaction.
pub async fn valider(
    tx: &mut TransactionLocataire,
    role: Role,
    document_id: Uuid,
) -> Resultat<RapportValidation> {
    let utilisateur_id = tx.utilisateur_id();
    let conn = tx.conn();

    let doc = lire(&mut *conn, document_id).await?;
    if !doc.etat.modifiable() {
        return Err(domaine::ErreurMetier::DocumentNonModifiable.into());
    }

    let lignes_a_traiter = sqlx::query!(
        r#"select ld.id, ld.fut_id from ligne_document ld
            where ld.document_id = $1 order by ld.fut_id"#,
        document_id
    )
    .fetch_all(&mut *conn)
    .await?;

    let mut traites = 0usize;
    for ligne in &lignes_a_traiter {
        let demande = demande_depuis(&doc, ligne.fut_id);
        let evenement_id = evenement::enregistrer(tx, role, &demande).await?;
        sqlx::query!(
            "update ligne_document set evenement_id = $2 where id = $1",
            ligne.id,
            evenement_id
        )
        .execute(tx.conn())
        .await?;
        traites += 1;
    }

    sqlx::query!(
        r#"update document_mouvement
              set etat = 'valide', valide_par = $2, valide_le = now(), modifie_le = now()
            where id = $1"#,
        document_id,
        utilisateur_id
    )
    .execute(tx.conn())
    .await?;

    Ok(RapportValidation {
        reference: doc.reference,
        futs: traites,
        statut_cible: doc.statut_cible,
    })
}
