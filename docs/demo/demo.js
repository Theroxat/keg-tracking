// Démonstration statique : seul comportement simulé, le scan par saisie de code.
// Aucun appel réseau, rien n'est enregistré.
(function () {
  'use strict';

  var retour = document.querySelector('[data-demo-retour]');
  if (retour) retour.addEventListener('click', function () { history.back(); });

  var racine = document.getElementById('scan');
  var form = document.getElementById('scan-manuel');
  if (!racine || !form) return;

  var connus = (racine.getAttribute('data-demo-codes') || '').split(',').filter(Boolean);
  var elRetour = document.getElementById('scan-retour');
  var elJournal = document.getElementById('scan-journal');
  var minuteur = null;

  function fermer() {
    clearTimeout(minuteur);
    elRetour.setAttribute('data-etat', '');
    elRetour.innerHTML = '';
  }

  function montrer(etat, titre, detail) {
    clearTimeout(minuteur);
    elRetour.setAttribute('data-etat', etat);
    elRetour.setAttribute('data-mode', 'unitaire');
    elRetour.innerHTML = '';
    var fort = document.createElement('div');
    fort.className = 'retour__titre';
    fort.textContent = titre;
    elRetour.appendChild(fort);
    var petit = document.createElement('small');
    petit.textContent = detail;
    elRetour.appendChild(petit);
    if (etat === 'succes') {
      minuteur = setTimeout(fermer, 1200);
    } else {
      var suite = document.createElement('p');
      suite.className = 'retour__suite';
      suite.textContent = 'Tap to continue';
      elRetour.appendChild(suite);
    }
  }

  function journaliser(code, etat, texte) {
    if (!elJournal) return;
    var vide = elJournal.querySelector('[data-vide]');
    if (vide) vide.remove();
    var li = document.createElement('li');
    var pastille = document.createElement('span');
    pastille.className = 'pastille';
    pastille.style.background = etat === 'succes' ? 'var(--ok)' : 'var(--grave)';
    var c = document.createElement('span');
    c.className = 'code';
    c.textContent = code;
    var d = document.createElement('span');
    d.className = 'petit discret';
    d.textContent = texte;
    li.append(pastille, c, d);
    elJournal.prepend(li);
  }

  elRetour.addEventListener('click', function () {
    if (elRetour.getAttribute('data-etat') === 'echec') fermer();
  });

  form.addEventListener('submit', function (e) {
    e.preventDefault();
    var champ = form.elements.code_qr;
    var code = (champ.value || '').trim().toUpperCase();
    if (!code) return;
    if (connus.indexOf(code) >= 0) {
      montrer('succes', 'OK ' + code, 'Demo: nothing is saved.');
      journaliser(code, 'succes', 'Demo: nothing is saved.');
    } else {
      montrer('echec', code, 'Unknown code. Try ' + (connus[0] || 'QA00001') + '.');
      journaliser(code, 'echec', 'Unknown code');
    }
    champ.value = '';
  });
})();
