// Pavé PIN : saisie au gros doigt, sans clavier système. Le serveur reste
// seul juge : ce script remplit le champ du formulaire et le soumet.
(function () {
  "use strict";
  var pave = document.getElementById("pin-pave");
  var champ = document.getElementById("champ-pin");
  if (!pave || !champ) return;
  var form = champ.form;
  var repli = document.getElementById("pin-repli");
  var valider = document.getElementById("pin-valider");
  var points = pave.querySelectorAll(".pin-points span");
  var LONGUEUR = 6;
  var envoye = false;

  pave.hidden = false;
  repli.hidden = true;
  valider.hidden = true;
  champ.required = false;

  function maj() {
    for (var i = 0; i < points.length; i++) {
      points[i].setAttribute("data-rempli", i < champ.value.length ? "1" : "0");
    }
  }
  function ajouter(c) {
    if (envoye || champ.value.length >= LONGUEUR) return;
    champ.value += c;
    maj();
    if (champ.value.length === LONGUEUR) {
      envoye = true;
      if (form.requestSubmit) form.requestSubmit(); else form.submit();
    }
  }
  function retirer() {
    if (envoye) return;
    champ.value = champ.value.slice(0, -1);
    maj();
  }

  pave.addEventListener("click", function (e) {
    var b = e.target.closest("button");
    if (!b) return;
    if (b.hasAttribute("data-effacer")) retirer();
    else ajouter(b.getAttribute("data-chiffre"));
  });
  // Clavier physique ou douchette : mêmes effets que le pavé.
  document.addEventListener("keydown", function (e) {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (/^[0-9]$/.test(e.key)) { e.preventDefault(); ajouter(e.key); }
    else if (e.key === "Backspace") { e.preventDefault(); retirer(); }
  });
  maj();
})();
