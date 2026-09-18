-- Usurpation de support.
--
-- Se connecter en tant que client pour diagnostiquer est utile, mais c'est
-- un pouvoir dangereux : la session porte donc une marque indélébile, elle
-- est journalisée à l'ouverture comme à la fermeture, et l'interface
-- affiche un bandeau permanent tant qu'elle dure.

alter table session
  add column usurpee_par uuid references super_admin(id) on delete set null;

create index on session (usurpee_par) where usurpee_par is not null;

comment on column session.usurpee_par is
  'Compte éditeur ayant ouvert cette session en assistance. Non nul = session usurpée.';
