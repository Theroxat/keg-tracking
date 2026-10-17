/* ==========================================================================
   Îlot de scan — JavaScript autonome, sans framework et sans étape de
   construction.

   Le serveur reste seul détenteur de l'état : ce fichier capture un code et
   l'envoie, il ne décide de rien. C'est ce qui permet de ne pas construire
   de couche hors-ligne.

   Quatre entrées possibles, par ordre de préférence sur le terrain :
     1. douchette Bluetooth (se comporte comme un clavier) — la plus rapide ;
     2. caméra via BarcodeDetector natif ;
     3. caméra via zxing-wasm (Safari, Firefox) ;
     4. saisie manuelle (étiquette abîmée, caméra indisponible).
   ========================================================================== */
(() => {
  'use strict';

  const racine = document.getElementById('scan');
  if (!racine) return;

  const URL_ENVOI = racine.dataset.url;
  const CSRF = racine.dataset.csrf || '';
  const RELANCE_MS = parseInt(racine.dataset.relance || '900', 10);
  const CIBLE_RAFRAICHIR = racine.dataset.rafraichir || '';

  const elVideo = document.getElementById('scan-video');
  const elRetour = document.getElementById('scan-retour');
  const elJournal = document.getElementById('scan-journal');
  const elDouchette = document.getElementById('champ-douchette');
  const elEtatDecodeur = document.getElementById('scan-decodeur');
  const elHors = document.getElementById('scan-hors');
  const elCompteur = document.getElementById('scan-compteur');
  const elManuel = document.getElementById('scan-manuel');
  const elBasculer = document.getElementById('scan-basculer');
  const elTorche = document.getElementById('scan-torche');

  const t = (cle) => racine.dataset[cle] || '';

  // ---------------------------------------------------------------- retour
  // Couleur plein écran, son et vibration : l'opérateur ne regarde pas
  // l'écran en continu, et il a du bruit autour de lui.
  let ctxAudio = null;
  function bip(frequences, duree) {
    try {
      ctxAudio = ctxAudio || new (window.AudioContext || window.webkitAudioContext)();
      if (ctxAudio.state === 'suspended') ctxAudio.resume();
      let debut = ctxAudio.currentTime;
      for (const f of frequences) {
        const osc = ctxAudio.createOscillator();
        const gain = ctxAudio.createGain();
        osc.type = 'square';
        osc.frequency.value = f;
        gain.gain.setValueAtTime(0.0001, debut);
        gain.gain.exponentialRampToValueAtTime(0.16, debut + 0.008);
        gain.gain.exponentialRampToValueAtTime(0.0001, debut + duree);
        osc.connect(gain).connect(ctxAudio.destination);
        osc.start(debut);
        osc.stop(debut + duree);
        debut += duree;
      }
    } catch (_) { /* le son est un confort, jamais un prérequis */ }
  }

  function vibrer(motif) {
    try { navigator.vibrate && navigator.vibrate(motif); } catch (_) {}
  }

  let minuteurRetour = null;
  function montrer(etat, titre, detail) {
    if (!elRetour) return;
    elRetour.dataset.etat = etat;
    elRetour.innerHTML = '';
    const fort = document.createElement('div');
    fort.textContent = titre;
    elRetour.appendChild(fort);
    if (detail) {
      const petit = document.createElement('small');
      petit.textContent = detail;
      elRetour.appendChild(petit);
    }
    if (etat === 'succes') { bip([880], 0.07); vibrer(40); }
    else if (etat === 'echec') { bip([300, 220], 0.1); vibrer([70, 60, 70]); }
    else { bip([620], 0.09); vibrer([30, 40, 30]); }

    clearTimeout(minuteurRetour);
    minuteurRetour = setTimeout(() => {
      elRetour.dataset.etat = '';
      reprendre();
    }, RELANCE_MS);
  }

  // ---------------------------------------------------------------- journal
  function journaliser(code, etat, texte) {
    if (!elJournal) return;
    const vide = elJournal.querySelector('[data-vide]');
    if (vide) vide.remove();
    const li = document.createElement('li');
    const pastille = document.createElement('span');
    pastille.className = 'pastille';
    pastille.style.background = etat === 'succes' ? 'var(--ok)'
      : etat === 'echec' ? 'var(--grave)' : 'var(--alerte)';
    const c = document.createElement('span');
    c.className = 'code';
    c.textContent = code;
    const d = document.createElement('span');
    d.className = 'petit discret';
    d.textContent = texte || '';
    li.append(pastille, c, d);
    elJournal.prepend(li);
    while (elJournal.children.length > 12) elJournal.lastElementChild.remove();
  }

  // ---------------------------------------------------------------- envoi
  // File sérialisée : deux lectures rapprochées ne partent pas en parallèle
  // et ne produisent donc pas de doublon.
  let enCours = Promise.resolve();
  let derniereValeur = '';
  let derniereHeure = 0;

  function envoyer(code) {
    code = (code || '').trim();
    if (!code) return;
    // Anti-rebond : le même code relu dans la foulée est ignoré.
    const maintenant = Date.now();
    if (code === derniereValeur && maintenant - derniereHeure < RELANCE_MS) return;
    derniereValeur = code;
    derniereHeure = maintenant;

    suspendre();
    enCours = enCours.then(() => fetch(URL_ENVOI, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-CSRF': CSRF },
      body: JSON.stringify({ code_qr: code }),
    }).then((r) => r.json().catch(() => ({
      resultat: 'echec', code, titre: t('libelleErreurReseau'),
    }))).then((rep) => {
      montrer(rep.resultat || 'echec', rep.titre || code, rep.detail || '');
      journaliser(rep.code || code, rep.resultat || 'echec', rep.detail || rep.titre || '');
      if (elCompteur && typeof rep.compteur === 'number') {
        elCompteur.textContent = rep.compteur;
      }
      if (rep.rafraichir && CIBLE_RAFRAICHIR && window.htmx) {
        window.htmx.ajax('GET', rep.rafraichir, { target: CIBLE_RAFRAICHIR, swap: 'innerHTML' });
      }
      if (rep.rediriger) {
        clearTimeout(minuteurRetour);
        setTimeout(() => { window.location.href = rep.rediriger; }, 320);
      }
    }).catch(() => {
      montrer('echec', t('libelleErreurReseau') || 'Erreur réseau', '');
    }));
  }

  // ---------------------------------------------------------------- douchette
  // Une douchette Bluetooth 2D se comporte exactement comme un clavier :
  // elle tape le code puis envoie Entrée. Quelques dizaines de lignes, et
  // l'application devient utilisable avec du vrai matériel de terrain.
  const SEUIL_CADENCE_MS = 35;   // intervalle médian au-delà duquel c'est humain
  const MIN_CARACTERES = 4;
  const DELAI_MAX_SAISIE = 900;  // au-delà, on repart de zéro

  let tampon = '';
  let horodatages = [];
  let derniereTouche = 0;  // heure de la dernière touche retenue

  function mediane(xs) {
    if (!xs.length) return Infinity;
    const tri = [...xs].sort((a, b) => a - b);
    const m = Math.floor(tri.length / 2);
    return tri.length % 2 ? tri[m] : (tri[m - 1] + tri[m]) / 2;
  }

  function estUneDouchette() {
    if (tampon.length < MIN_CARACTERES) return false;
    const ecarts = [];
    for (let i = 1; i < horodatages.length; i++) {
      ecarts.push(horodatages[i] - horodatages[i - 1]);
    }
    return mediane(ecarts) < SEUIL_CADENCE_MS;
  }

  function surTouche(e) {
    // Ne jamais voler le clavier d'un champ où l'on saisit vraiment.
    const actif = document.activeElement;
    const dansUnChamp = actif && actif !== elDouchette &&
      (actif.tagName === 'INPUT' || actif.tagName === 'TEXTAREA' || actif.tagName === 'SELECT');
    if (dansUnChamp) return;

    const maintenant = performance.now();
    if (maintenant - derniereTouche > DELAI_MAX_SAISIE) {
      tampon = ''; horodatages = []; derniereTouche = maintenant;
    }

    if (e.key === 'Enter') {
      if (tampon.length >= MIN_CARACTERES) {
        const rapide = estUneDouchette();
        temoinDouchette(rapide);
        envoyer(tampon);
        e.preventDefault();
      }
      tampon = ''; horodatages = [];
      return;
    }
    if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      tampon += e.key;
      horodatages.push(maintenant);
      derniereTouche = maintenant;
    }
  }

  let temoinVu = false;
  function temoinDouchette(actif) {
    const el = document.getElementById('douchette-temoin');
    if (el && actif && !temoinVu) { el.classList.remove('rien'); temoinVu = true; }
  }

  // Le champ invisible garde le focus pour que la douchette trouve toujours
  // une cible, y compris après un clic ailleurs sur la page.
  function reprendreFocus() {
    if (!elDouchette) return;
    const actif = document.activeElement;
    if (actif && actif !== document.body && actif !== elDouchette &&
        ['INPUT', 'TEXTAREA', 'SELECT', 'BUTTON', 'A'].includes(actif.tagName)) return;
    try { elDouchette.focus({ preventScroll: true }); } catch (_) {}
  }

  document.addEventListener('keydown', surTouche, true);
  document.addEventListener('click', () => setTimeout(reprendreFocus, 0));
  setInterval(reprendreFocus, 1500);
  reprendreFocus();

  // ---------------------------------------------------------------- décodeur
  let decodeur = null;     // { lire(bitmapOuCanvas) -> string|null }
  let detecteurNatif = null;

  async function choisirDecodeur() {
    // BarcodeDetector natif : rapide et économe en batterie (Chrome/Android).
    if ('BarcodeDetector' in window) {
      try {
        const formats = await window.BarcodeDetector.getSupportedFormats();
        if (formats.includes('qr_code')) {
          detecteurNatif = new window.BarcodeDetector({ formats: ['qr_code'] });
          etiqueterDecodeur(t('libelleNatif'));
          return async (source) => {
            const codes = await detecteurNatif.detect(source);
            return codes.length ? codes[0].rawValue : null;
          };
        }
      } catch (_) { /* on bascule sur le repli */ }
    }
    // Repli WASM, chargé seulement s'il sert : Safari et Firefox.
    await chargerScript(racine.dataset.zxingJs);
    const mod = window.ZXingWASM;
    mod.prepareZXingModule({
      overrides: { locateFile: (f) => (f.endsWith('.wasm') ? racine.dataset.zxingWasm : f) },
    });
    etiqueterDecodeur(t('libelleWasm'));
    return async (source) => {
      const canvas = versCanvas(source);
      const ctx = canvas.getContext('2d', { willReadFrequently: true });
      const image = ctx.getImageData(0, 0, canvas.width, canvas.height);
      const res = await mod.readBarcodes(image, { formats: ['QRCode'], tryHarder: false });
      return res && res.length ? res[0].text : null;
    };
  }

  function etiqueterDecodeur(texte) {
    if (elEtatDecodeur && texte) elEtatDecodeur.textContent = texte;
  }

  function chargerScript(src) {
    return new Promise((ok, ko) => {
      const s = document.createElement('script');
      s.src = src; s.onload = ok; s.onerror = ko;
      document.head.appendChild(s);
    });
  }

  let canvasTampon = null;
  function versCanvas(video) {
    const l = video.videoWidth || 640, h = video.videoHeight || 480;
    // On ne décode qu'un carré central : c'est là que vise l'opérateur, et
    // cela divise par trois le travail du décodeur logiciel.
    const cote = Math.floor(Math.min(l, h) * 0.75);
    canvasTampon = canvasTampon || document.createElement('canvas');
    canvasTampon.width = cote; canvasTampon.height = cote;
    const ctx = canvasTampon.getContext('2d', { willReadFrequently: true });
    ctx.drawImage(video, (l - cote) / 2, (h - cote) / 2, cote, cote, 0, 0, cote, cote);
    return canvasTampon;
  }

  // ---------------------------------------------------------------- caméra
  let flux = null;
  let piste = null;
  let camerasArriere = true;
  let boucle = null;
  let enPause = false;

  function suspendre() { enPause = true; }
  function reprendre() { enPause = false; }

  function afficherHors(message) {
    if (!elHors) return;
    elHors.textContent = message;
    elHors.classList.remove('rien');
    // Sans caméra, les contrôles qui la pilotent n'ont plus de sens, et le
    // témoin de décodeur ne doit pas rester bloqué sur « Chargement… ».
    elBasculer && elBasculer.classList.add('rien');
    elTorche && elTorche.classList.add('rien');
    etiqueterDecodeur(t('libelleSaisieSeule'));
    const champ = elManuel && elManuel.querySelector('input[name=code_qr]');
    champ && champ.focus({ preventScroll: true });
  }

  async function demarrerCamera() {
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
      afficherHors(window.isSecureContext ? t('libelleCameraIndisponible')
                                          : t('libelleContexteNonSecurise'));
      return;
    }
    if (!window.isSecureContext) {
      // getUserMedia exige un contexte sécurisé. C'est le blocage qui se
      // découvre en général le jour de la démonstration.
      afficherHors(t('libelleContexteNonSecurise'));
      return;
    }
    try {
      arreterCamera();
      flux = await navigator.mediaDevices.getUserMedia({
        video: {
          facingMode: camerasArriere ? { ideal: 'environment' } : 'user',
          width: { ideal: 1280 }, height: { ideal: 960 },
        },
        audio: false,
      });
      elVideo.srcObject = flux;
      await elVideo.play();
      piste = flux.getVideoTracks()[0];
      if (elHors) elHors.classList.add('rien');
      elBasculer && elBasculer.classList.remove('rien');
      preparerTorche();
      if (!decodeur) decodeur = await choisirDecodeur();
      lancerBoucle();
    } catch (e) {
      afficherHors(e && e.name === 'NotAllowedError'
        ? t('libelleCameraRefusee') : t('libelleCameraIndisponible'));
    }
  }

  function arreterCamera() {
    cancelAnimationFrame(boucle);
    if (flux) flux.getTracks().forEach((p) => p.stop());
    flux = null; piste = null;
  }

  function preparerTorche() {
    if (!elTorche || !piste) return;
    const cap = piste.getCapabilities ? piste.getCapabilities() : {};
    if (cap && 'torch' in cap) elTorche.classList.remove('rien');
    else elTorche.classList.add('rien');
  }

  let torcheAllumee = false;
  elTorche && elTorche.addEventListener('click', async () => {
    if (!piste) return;
    torcheAllumee = !torcheAllumee;
    try { await piste.applyConstraints({ advanced: [{ torch: torcheAllumee }] }); } catch (_) {}
    elTorche.setAttribute('aria-pressed', String(torcheAllumee));
  });

  elBasculer && elBasculer.addEventListener('click', () => {
    camerasArriere = !camerasArriere;
    demarrerCamera();
  });

  let dernierDecodage = 0;
  function lancerBoucle() {
    const tick = async () => {
      boucle = requestAnimationFrame(tick);
      if (enPause || !decodeur || !elVideo || elVideo.readyState < 2) return;
      const maintenant = performance.now();
      if (maintenant - dernierDecodage < 120) return; // ~8 lectures/seconde
      dernierDecodage = maintenant;
      try {
        const valeur = await decodeur(elVideo);
        if (valeur) envoyer(valeur);
      } catch (_) { /* une image ratée n'est pas un incident */ }
    };
    boucle = requestAnimationFrame(tick);
  }

  // Économie de batterie : on coupe la caméra quand l'onglet passe en fond.
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) arreterCamera();
    else if (racine.dataset.camera !== 'non') demarrerCamera();
  });
  window.addEventListener('pagehide', arreterCamera);

  // ---------------------------------------------------------------- manuel
  elManuel && elManuel.addEventListener('submit', (e) => {
    e.preventDefault();
    const champ = elManuel.querySelector('input[name=code_qr]');
    if (champ && champ.value.trim()) {
      envoyer(champ.value);
      champ.value = '';
      setTimeout(reprendreFocus, 50);
    }
  });

  // ---------------------------------------------------------------- départ
  // La caméra arrière démarre automatiquement sur mobile ; la douchette est
  // active dès le premier instant, sans rien à activer.
  if (racine.dataset.camera !== 'non') demarrerCamera();
  else choisirDecodeur().then((d) => { decodeur = d; }).catch(() => {});
})();
