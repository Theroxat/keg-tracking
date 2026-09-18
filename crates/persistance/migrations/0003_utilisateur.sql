create table utilisateur (
  id                    uuid primary key default gen_random_uuid(),
  tenant_id             uuid        not null references locataire(id) on delete cascade,
  email                 citext      not null,
  mot_de_passe_hash     text        not null,
  pin_hash              text,
  nom                   text        not null,
  prenom                text        not null default '',
  role                  role_utilisateur not null,
  actif                 boolean     not null default true,
  derniere_connexion_le timestamptz,
  echecs_connexion      int         not null default 0,
  verrouille_jusqu_a    timestamptz,
  cree_le               timestamptz not null default now(),
  modifie_le            timestamptz not null default now(),
  unique (tenant_id, email)
);
-- Un compte appartient à exactement une brasserie, et l'écran de connexion
-- ne demande pas laquelle : l'adresse doit donc être unique globalement.
create unique index utilisateur_email_global on utilisateur (email);
create index on utilisateur (tenant_id, actif);

create table session (
  id                   uuid primary key default gen_random_uuid(),
  tenant_id            uuid        not null references locataire(id) on delete cascade,
  utilisateur_id       uuid        not null references utilisateur(id) on delete cascade,
  jeton_hash           bytea       not null unique,
  jeton_csrf           text        not null,
  appareil_libelle     text,
  ip_creation          inet,
  cree_le              timestamptz not null default now(),
  expire_le            timestamptz not null,
  derniere_activite_le timestamptz not null default now(),
  verrouillee          boolean     not null default false,
  echecs_pin           int         not null default 0,
  pin_bloque_jusqu_a   timestamptz,
  revoquee_le          timestamptz
);
create index on session (tenant_id, utilisateur_id);
create index on session (expire_le);

create table journal_audit (
  id             bigserial primary key,
  tenant_id      uuid        not null references locataire(id) on delete cascade,
  utilisateur_id uuid references utilisateur(id) on delete set null,
  action         text        not null,
  cible_type     text,
  cible_id       text,
  details        jsonb       not null default '{}'::jsonb,
  ip             inet,
  survenu_le     timestamptz not null default now()
);
create index on journal_audit (tenant_id, survenu_le desc);

-- Numérotation des documents : par locataire, par clé, sous verrou de ligne.
create table compteur_reference (
  tenant_id uuid   not null references locataire(id) on delete cascade,
  cle       text   not null,
  valeur    bigint not null default 0,
  primary key (tenant_id, cle)
);

select appliquer_rls();
