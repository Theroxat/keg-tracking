#!/bin/bash
# Crée les trois rôles applicatifs et installe les extensions dans template1,
# afin que toute base créée ensuite (y compris les bases jetables des tests)
# en hérite. Exécuté une seule fois, à l'initialisation du volume.
set -euo pipefail

: "${MDP_OWNER:?MDP_OWNER manquant}"
: "${MDP_APP:?MDP_APP manquant}"
: "${MDP_EDITEUR:?MDP_EDITEUR manquant}"
: "${MDP_AUTH:?MDP_AUTH manquant}"

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname postgres <<SQL
do \$\$ begin
  if not exists (select 1 from pg_roles where rolname = 'webqr_owner') then
    create role webqr_owner login createdb password '${MDP_OWNER}';
  end if;
  if not exists (select 1 from pg_roles where rolname = 'webqr_app') then
    create role webqr_app login password '${MDP_APP}';
  end if;
  if not exists (select 1 from pg_roles where rolname = 'webqr_editeur') then
    create role webqr_editeur login password '${MDP_EDITEUR}';
  end if;
  if not exists (select 1 from pg_roles where rolname = 'webqr_auth') then
    create role webqr_auth login password '${MDP_AUTH}';
  end if;
end \$\$;

-- Aucun de ces rôles n'est superutilisateur et aucun ne porte BYPASSRLS.
alter role webqr_app     nosuperuser nocreatedb nocreaterole nobypassrls;
alter role webqr_editeur nosuperuser nocreatedb nocreaterole nobypassrls;
alter role webqr_auth    nosuperuser nocreatedb nocreaterole nobypassrls;
alter role webqr_owner   nosuperuser nobypassrls;
SQL

for base in template1 "$POSTGRES_DB"; do
  psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$base" <<SQL
create extension if not exists citext;
create extension if not exists pg_trgm;
SQL
done

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname postgres <<SQL
alter database "$POSTGRES_DB" owner to webqr_owner;
SQL

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<SQL
alter schema public owner to webqr_owner;
grant usage on schema public to webqr_app, webqr_editeur, webqr_auth;
SQL

echo "rôles webqr_owner / webqr_app / webqr_editeur / webqr_auth prêts"
