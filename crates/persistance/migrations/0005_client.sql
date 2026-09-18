create table client (
  id             uuid primary key default gen_random_uuid(),
  tenant_id      uuid        not null references locataire(id) on delete cascade,
  societe        text        not null check (length(trim(societe)) > 0),
  contact_nom    text        not null default '',
  contact_prenom text        not null default '',
  telephone      text        not null default '',
  email          text        not null default '',
  commentaire    text        not null default '',
  archive        boolean     not null default false,
  cree_le        timestamptz not null default now(),
  modifie_le     timestamptz not null default now()
);
create index on client (tenant_id, archive);
create index on client using gin (societe gin_trgm_ops);

create table adresse_client (
  id          uuid primary key default gen_random_uuid(),
  tenant_id   uuid        not null references locataire(id) on delete cascade,
  client_id   uuid        not null references client(id) on delete cascade,
  libelle     text        not null default 'Principale',
  rue         text        not null default '',
  code_postal text        not null default '',
  ville       text        not null default '',
  archive     boolean     not null default false,
  cree_le     timestamptz not null default now(),
  -- Permet aux tables qui référencent un couple client + adresse de poser une
  -- clé étrangère composite : « l'adresse appartient au client » devient
  -- structurellement impossible à violer, même par une requête forgée.
  unique (tenant_id, client_id, id)
);
create index on adresse_client (tenant_id, client_id);

select appliquer_rls();
