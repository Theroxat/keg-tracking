-- Types fermés et outillage transverse.
-- Les valeurs sont sans accent : l'accent n'existe que dans le catalogue de
-- traduction. C'est ce qui élimine la classe de bugs de la version précédente.

create type statut_fut as enum
  ('recu', 'sale', 'propre', 'rempli', 'livre', 'hors_service', 'perdu');

create type role_utilisateur as enum
  ('lecture', 'production', 'administrateur');

create type type_evenement as enum
  ('mise_en_service', 'transition', 'transition_forcee', 'correction',
   'modification_donnees', 'import');

create type origine_evenement as enum
  ('scan', 'document', 'edition_manuelle', 'import', 'systeme');

create type type_document as enum
  ('livraison', 'retour', 'operation_interne');

create type etat_document as enum
  ('brouillon', 'valide', 'abandonne');

create type etat_locataire as enum ('actif', 'suspendu');

-- Le locataire courant, positionné par SET LOCAL au début de chaque
-- transaction applicative. Non positionné => NULL => aucune ligne.
create or replace function locataire_courant() returns uuid
  language sql stable
  as $$ select nullif(current_setting('app.locataire_id', true), '')::uuid $$;

create or replace function utilisateur_courant() returns uuid
  language sql stable
  as $$ select nullif(current_setting('app.utilisateur_id', true), '')::uuid $$;

-- Applique la politique d'isolation à **toute** table portant une colonne
-- tenant_id. Appelée à la fin de chaque migration qui crée des tables : une
-- table métier ne peut donc pas exister sans sa politique.
create or replace function appliquer_rls() returns void
language plpgsql as $fn$
declare t text;
begin
  for t in
    select c.relname
      from pg_class c
      join pg_namespace n on n.oid = c.relnamespace
      join pg_attribute a on a.attrelid = c.oid
                         and a.attname = 'tenant_id'
                         and not a.attisdropped
     where n.nspname = 'public' and c.relkind = 'r'
  loop
    execute format('alter table %I enable row level security', t);

    execute format('drop policy if exists isolation_locataire on %I', t);
    execute format(
      'create policy isolation_locataire on %I for all to webqr_app '
      'using (tenant_id = locataire_courant()) '
      'with check (tenant_id = locataire_courant())', t);

    execute format('drop policy if exists acces_editeur on %I', t);
    execute format(
      'create policy acces_editeur on %I for all to webqr_editeur '
      'using (true) with check (true)', t);

    execute format(
      'grant select, insert, update, delete on %I to webqr_app, webqr_editeur', t);
  end loop;

  -- Séquences éventuelles.
  execute 'grant usage, select on all sequences in schema public to webqr_app, webqr_editeur';
end
$fn$;

-- Refuse toute réécriture de l'histoire, quel que soit le rôle.
create or replace function interdire_modification() returns trigger
language plpgsql as $fn$
begin
  raise exception
    'table % en ajout seul : une erreur se corrige par un nouvel evenement',
    tg_table_name
    using errcode = 'restrict_violation';
end
$fn$;
