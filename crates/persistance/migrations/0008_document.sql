-- Le document de mouvement : une livraison, un retour, une opération interne.
-- Le brouillon vit côté serveur dès le premier scan.

create table document_mouvement (
  id                uuid primary key default gen_random_uuid(),
  tenant_id         uuid        not null references locataire(id) on delete cascade,
  type_document     type_document not null,
  reference         text        not null,
  etat              etat_document not null default 'brouillon',

  -- configuration à appliquer aux fûts capturés
  statut_cible      statut_fut,
  type_biere_id     uuid references type_biere(id) on delete set null,
  lot_id            uuid references lot(id) on delete set null,
  cuve_id           uuid references cuve(id) on delete set null,
  emplacement_id    uuid references emplacement(id) on delete set null,
  client_id         uuid,
  adresse_client_id uuid,
  commentaire       text        not null default '',

  cree_par          uuid references utilisateur(id) on delete set null,
  cree_le           timestamptz not null default now(),
  modifie_le        timestamptz not null default now(),
  valide_par        uuid references utilisateur(id) on delete set null,
  valide_le         timestamptz,

  unique (tenant_id, reference),
  constraint document_client_adresse_ensemble
    check ((client_id is null) = (adresse_client_id is null)),
  constraint document_adresse_du_client_fk
    foreign key (tenant_id, client_id, adresse_client_id)
    references adresse_client (tenant_id, client_id, id) on delete set null
);
create index on document_mouvement (tenant_id, etat, modifie_le desc);
create index on document_mouvement (tenant_id, cree_par, etat);

create table ligne_document (
  id                  uuid primary key default gen_random_uuid(),
  tenant_id           uuid        not null references locataire(id) on delete cascade,
  document_id         uuid        not null references document_mouvement(id) on delete cascade,
  fut_id              uuid        not null references fut(id) on delete cascade,
  statut_avant_capture statut_fut not null,
  capture_le          timestamptz not null default now(),
  capture_par         uuid references utilisateur(id) on delete set null,
  evenement_id        uuid references evenement_fut(id) on delete set null,
  ordre               int         not null default 0,
  -- Le dédoublonnage est garanti par la base, pas seulement par l'interface.
  unique (tenant_id, document_id, fut_id)
);
create index on ligne_document (tenant_id, document_id, ordre);

alter table evenement_fut
  add constraint evenement_document_fk
  foreign key (document_id) references document_mouvement(id) on delete set null;

select appliquer_rls();
revoke update, delete on evenement_fut from webqr_app, webqr_editeur;
