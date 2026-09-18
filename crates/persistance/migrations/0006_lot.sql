create table lot (
  id            uuid primary key default gen_random_uuid(),
  tenant_id     uuid        not null references locataire(id) on delete cascade,
  numero        text        not null check (length(trim(numero)) > 0),
  date_brassage date        not null,
  type_biere_id uuid        not null references type_biere(id) on delete restrict,
  cuve_id       uuid        not null references cuve(id) on delete restrict,
  dluo          date,
  commentaire   text        not null default '',
  cree_le       timestamptz not null default now(),
  unique (tenant_id, numero)
);
create index on lot (tenant_id, date_brassage desc);
create index on lot (tenant_id, dluo);
create index on lot using gin (numero gin_trgm_ops);

select appliquer_rls();
