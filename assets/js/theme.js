// Applique le thème enregistré avant le premier rendu, pour éviter un éclair
// sombre chez ceux qui ont choisi le clair. Chargé de façon bloquante dans le
// <head> : ce fichier doit rester minuscule. Le thème sombre est le défaut.
(function () {
  "use strict";
  var clair = false;
  try {
    clair = window.localStorage.getItem("theme") === "clair";
  } catch (_) {
    /* stockage indisponible : on reste sur le défaut */
  }
  if (clair) document.documentElement.dataset.theme = "clair";
})();
