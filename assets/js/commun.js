// Comportements communs, par délégation d'événements. La CSP interdit tout
// JavaScript en ligne : les gabarits déclarent des attributs data-*.
//  - data-confirmer="message"  sur un <form> ou sur un bouton d'envoi
//  - data-role-initial / data-actif-initial / data-confirmer-modif  sur la fiche utilisateur
//  - touche « / »  focalise #recherche-globale
//  - data-imprimer  sur un bouton : ouvre la boîte d'impression
(function () {
  "use strict";

  document.addEventListener("submit", function (e) {
    var f = e.target;
    var b = e.submitter;
    if (b && b.dataset && b.dataset.confirmer) {
      if (!window.confirm(b.dataset.confirmer)) e.preventDefault();
      return;
    }
    if (f.dataset && f.dataset.confirmer) {
      if (!window.confirm(f.dataset.confirmer)) e.preventDefault();
      return;
    }
    if (f.dataset && f.dataset.confirmerModif) {
      var r = f.elements["role"];
      var a = f.elements["actif"];
      var role_change = r && r.value !== f.dataset.roleInitial;
      var desactive = f.dataset.actifInitial === "1" && a && !a.checked;
      if ((role_change || desactive) && !window.confirm(f.dataset.confirmerModif)) {
        e.preventDefault();
      }
    }
  });

  document.addEventListener("click", function (e) {
    var el = e.target.closest ? e.target.closest("[data-imprimer]") : null;
    if (el) window.print();
  });

  // « / » place le curseur dans la recherche globale (hors champ de saisie).
  document.addEventListener("keydown", function (e) {
    if (e.key !== "/" || e.ctrlKey || e.metaKey || e.altKey) return;
    var t = e.target;
    var tag = t && t.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || (t && t.isContentEditable)) return;
    var champ = document.getElementById("recherche-globale");
    if (champ && champ.offsetParent !== null) {
      e.preventDefault();
      champ.focus();
    }
  });

  document.body.addEventListener("htmx:responseError", function (e) {
    var s = e.detail.xhr.status;
    if (s === 401 || s === 403) window.location = "/connexion";
  });
})();
