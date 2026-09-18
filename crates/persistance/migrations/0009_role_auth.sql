-- Rôle d'authentification.
--
-- Résoudre un cookie de session ou vérifier un mot de passe impose de lire
-- avant de savoir de quel locataire il s'agit. Plutôt que d'élargir le rôle
-- éditeur — qui voit tout — on donne à ce seul besoin un rôle aux droits
-- strictement énumérés. Un défaut dans l'intergiciel de session ne peut donc
-- pas se transformer en lecture du parc d'une autre brasserie.

grant select on locataire   to webqr_auth;
grant select on utilisateur to webqr_auth;
-- Compteurs d'échec seulement : pas de droit sur le rôle ni sur les hachages.
grant update (echecs_connexion, verrouille_jusqu_a, derniere_connexion_le)
  on utilisateur to webqr_auth;
grant select, insert, update, delete on session to webqr_auth;

create policy acces_auth on locataire   for all to webqr_auth using (true) with check (true);
create policy acces_auth on utilisateur for all to webqr_auth using (true) with check (true);
create policy acces_auth on session     for all to webqr_auth using (true) with check (true);

-- Aucun accès au reste : le parc, les clients, les lots, les documents et
-- les événements lui sont fermés.
