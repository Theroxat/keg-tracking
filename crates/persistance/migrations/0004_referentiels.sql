-- Les trois référentiels. Utilisés partout en listes déroulantes, jamais en
-- saisie libre : c'était le deuxième défaut majeur de la version précédente.

create table type_biere (
  id         uuid primary key default gen_random_uuid(),
  tenant_id  uuid        not null references locataire(id) on delete cascade,
  nom        text        not null check (length(trim(nom)) > 0),
  couleur    text        not null default '#8A6A3E' check (couleur ~ '^#[0-9A-Fa-f]{6}$'),
  actif      boolean     not null default true,
  ordre      int         not null default 0,
  cree_le    timestamptz not null default now()
);
create unique index on type_biere (tenant_id, lower(nom));
create index on type_biere (tenant_id, actif, ordre);

create table emplacement (
  id         uuid primary key default gen_random_uuid(),
  tenant_id  uuid        not null references locataire(id) on delete cascade,
  nom        text        not null check (length(trim(nom)) > 0),
  actif      boolean     not null default true,
  cree_le    timestamptz not null default now()
);
create unique index on emplacement (tenant_id, lower(nom));

create table cuve (
  id         uuid primary key default gen_random_uuid(),
  tenant_id  uuid        not null references locataire(id) on delete cascade,
  nom        text        not null check (length(trim(nom)) > 0),
  actif      boolean     not null default true,
  cree_le    timestamptz not null default now()
);
create unique index on cuve (tenant_id, lower(nom));

alter table locataire
  add constraint locataire_emplacement_defaut_fk
  foreign key (emplacement_defaut_id) references emplacement(id) on delete set null;

select appliquer_rls();
