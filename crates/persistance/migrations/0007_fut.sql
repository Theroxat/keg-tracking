-- Le fût : son identité, et la projection de son histoire.
-- La projection est un cache. La vérité est `evenement_fut`.

create table fut (
  id                   uuid primary key default gen_random_uuid(),
  tenant_id            uuid        not null references locataire(id) on delete cascade,

  -- identité, immuable
  code_qr              text        not null check (length(trim(code_qr)) > 0),
  contenance_litres    int         not null check (contenance_litres > 0),
  date_mise_en_service date        not null default current_date,
  commentaire          text        not null default '',

  -- projection
  statut               statut_fut  not null default 'recu',
  emplacement_id       uuid references emplacement(id) on delete set null,
  client_id            uuid,
  adresse_client_id    uuid,
  type_biere_id        uuid references type_biere(id) on delete set null,
  lot_id               uuid references lot(id) on delete set null,
  cuve_id              uuid references cuve(id) on delete set null,
  date_remplissage     timestamptz,
  statut_depuis_le     timestamptz not null default now(),
  dernier_lavage_le    timestamptz,
  dernier_lavage_par   uuid references utilisateur(id) on delete set null,

  -- traçage
  dernier_evenement_id uuid,
  dernier_evenement_le timestamptz,
  sequence_courante    bigint      not null default 0,

  -- métadonnées
  cree_le              timestamptz not null default now(),
  cree_par             uuid references utilisateur(id) on delete set null,
  modifie_le           timestamptz not null default now(),
  modifie_par          uuid references utilisateur(id) on delete set null,

  unique (tenant_id, code_qr),

  -- Client et adresse vont toujours par deux.
  constraint fut_client_adresse_ensemble
    check ((client_id is null) = (adresse_client_id is null)),
  -- L'adresse appartient forcément au client : garanti par la base.
  constraint fut_adresse_du_client_fk
    foreign key (tenant_id, client_id, adresse_client_id)
    references adresse_client (tenant_id, client_id, id) on delete set null
);

create index on fut (tenant_id, statut);
create index on fut (tenant_id, client_id);
create index on fut (tenant_id, lot_id);
create index on fut (tenant_id, emplacement_id);
create index on fut (tenant_id, dernier_evenement_le desc);
create index on fut (tenant_id, statut, statut_depuis_le);
create index on fut using gin (code_qr gin_trgm_ops);

-- ------------------------------------------------------- flux d'événements
create table evenement_fut (
  id                uuid primary key,
  tenant_id         uuid        not null references locataire(id) on delete cascade,
  fut_id            uuid        not null references fut(id) on delete cascade,
  sequence          bigint      not null check (sequence > 0),

  type_evenement    type_evenement    not null,
  origine           origine_evenement not null,
  statut_avant      statut_fut,
  statut_apres      statut_fut,

  emplacement_id    uuid references emplacement(id) on delete set null,
  client_id         uuid,
  adresse_client_id uuid,
  type_biere_id     uuid references type_biere(id) on delete set null,
  lot_id            uuid references lot(id) on delete set null,
  cuve_id           uuid references cuve(id) on delete set null,

  commentaire       text        not null default '',
  justification     text        not null default '',
  document_id       uuid,
  utilisateur_id    uuid references utilisateur(id) on delete set null,
  survenu_le        timestamptz not null default now(),
  enregistre_le     timestamptz not null default now(),

  unique (tenant_id, fut_id, sequence),

  constraint evenement_forcee_justifiee
    check (type_evenement <> 'transition_forcee' or length(trim(justification)) > 0),
  constraint evenement_client_adresse_ensemble
    check ((client_id is null) = (adresse_client_id is null)),
  constraint evenement_adresse_du_client_fk
    foreign key (tenant_id, client_id, adresse_client_id)
    references adresse_client (tenant_id, client_id, id) on delete set null
);

create index on evenement_fut (tenant_id, fut_id, sequence);
create index on evenement_fut (tenant_id, survenu_le desc);
create index on evenement_fut (tenant_id, lot_id) where lot_id is not null;
create index on evenement_fut (tenant_id, client_id) where client_id is not null;
create index on evenement_fut (tenant_id, document_id) where document_id is not null;

-- Ajout seul, à deux niveaux : le trigger vaut pour tous les rôles,
-- le REVOKE (posé par appliquer_rls, puis restreint ci-dessous) pour l'appli.
create trigger evenement_fut_immuable
  before update or delete on evenement_fut
  for each row execute function interdire_modification();

select appliquer_rls();

-- appliquer_rls() accorde UPDATE/DELETE partout ; on les retire ici.
revoke update, delete on evenement_fut from webqr_app, webqr_editeur;
