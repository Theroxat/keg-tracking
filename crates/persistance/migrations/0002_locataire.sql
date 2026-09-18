-- Le locataire, et le périmètre éditeur qui vit hors de la RLS des locataires.

create table locataire (
  id                            uuid primary key default gen_random_uuid(),
  nom                           text        not null check (length(trim(nom)) > 0),
  identifiant_court             citext      not null unique
                                  check (identifiant_court ~ '^[a-z0-9][a-z0-9-]{1,38}[a-z0-9]$'),
  -- Unique globalement : deux brasseries ne peuvent pas émettre le même code.
  prefixe_qr                    text        not null unique
                                  check (prefixe_qr ~ '^[A-Z0-9]{2,12}$'),
  emplacement_defaut_id         uuid,
  fuseau_horaire                text        not null default 'Europe/Paris',
  seuil_alerte_non_revenu_jours int         not null default 30 check (seuil_alerte_non_revenu_jours > 0),
  seuil_alerte_dluo_jours       int         not null default 14 check (seuil_alerte_dluo_jours >= 0),
  contenances_autorisees        int[]       not null default '{20,30,50}',
  etat                          etat_locataire not null default 'actif',
  cree_le                       timestamptz not null default now(),
  modifie_le                    timestamptz not null default now()
);

-- Le locataire n'a pas de colonne tenant_id : sa politique lui est propre.
alter table locataire enable row level security;
create policy isolation_locataire on locataire for all to webqr_app
  using (id = locataire_courant()) with check (id = locataire_courant());
create policy acces_editeur on locataire for all to webqr_editeur
  using (true) with check (true);
grant select, insert, update, delete on locataire to webqr_app, webqr_editeur;

-- ---------------------------------------------------------------- éditeur
-- Table délibérément distincte de `utilisateur` : aucun compte locataire ne
-- peut être promu éditeur par une écriture malencontreuse.
create table super_admin (
  id                uuid primary key default gen_random_uuid(),
  email             citext      not null unique,
  mot_de_passe_hash text        not null,
  nom               text        not null,
  actif             boolean     not null default true,
  cree_le           timestamptz not null default now()
);

create table session_super_admin (
  id                   uuid primary key default gen_random_uuid(),
  super_admin_id       uuid        not null references super_admin(id) on delete cascade,
  jeton_hash           bytea       not null unique,
  jeton_csrf           text        not null,
  ip_creation          inet,
  agent                text,
  cree_le              timestamptz not null default now(),
  expire_le            timestamptz not null,
  derniere_activite_le timestamptz not null default now(),
  revoquee_le          timestamptz,
  -- Usurpation de support : toujours journalisée et visible.
  usurpe_locataire_id  uuid references locataire(id) on delete set null
);
create index on session_super_admin (super_admin_id);

create table journal_audit_editeur (
  id             bigserial primary key,
  super_admin_id uuid references super_admin(id) on delete set null,
  action         text        not null,
  locataire_id   uuid references locataire(id) on delete set null,
  details        jsonb       not null default '{}'::jsonb,
  ip             inet,
  survenu_le     timestamptz not null default now()
);
create index on journal_audit_editeur (survenu_le desc);

-- Le rôle applicatif des locataires n'a strictement aucun accès à ces tables.
revoke all on super_admin, session_super_admin, journal_audit_editeur from public;
grant select, insert, update, delete
  on super_admin, session_super_admin, journal_audit_editeur to webqr_editeur;
grant usage, select on sequence journal_audit_editeur_id_seq to webqr_editeur;
