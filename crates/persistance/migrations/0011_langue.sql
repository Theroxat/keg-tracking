-- Langue d'interface préférée de l'utilisateur.
--
-- Pas de nouvelle table : une colonne de `utilisateur`, déjà soumise à la
-- RLS. Les droits n'ont pas à être élargis :
--  - webqr_app lit et écrit la table entière (droit de table posé par
--    appliquer_rls(), qui couvre les colonnes ajoutées après coup) ;
--  - webqr_auth lit la table entière (0009) et en a besoin pour résoudre
--    la langue en même temps que la session ; il n'obtient en revanche
--    **aucun** droit d'écriture sur cette colonne : la préférence s'écrit
--    par une transaction locataire ordinaire.

alter table utilisateur
  add column langue text not null default 'fr'
    check (langue in ('fr', 'en'));

comment on column utilisateur.langue is
  'Code ISO 639-1 de la langue d''interface (fr, en).';
