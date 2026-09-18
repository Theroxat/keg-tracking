#!/usr/bin/env node
/**
 * Vérifie qu'aucune page ne déborde horizontalement sur téléphone.
 *
 * C'est la panne visuelle la plus fréquente, et la plus invisible depuis un
 * poste de bureau : une colonne de largeur fixe sans requête média, ou un
 * `1fr` — qui vaut `minmax(auto, 1fr)` — qu'un contenu insécable empêche de
 * rétrécir. Le symptôme est une page décalée ou une colonne écrasée à
 * quelques pixels ; aucun test unitaire ne l'attrape.
 *
 *   BASE=https://… EMAIL=… MDP=… node outils/audit-mobile.mjs
 *
 * Prévu pour tourner dans l'image officielle Playwright :
 *   docker run --rm --network host -v "$PWD":/w -w /w \
 *     -e BASE=… -e EMAIL=… -e MDP=… mcr.microsoft.com/playwright:v1.56.0-noble \
 *     sh -c 'npm i --no-save --silent playwright@1.56.0 && node outils/audit-mobile.mjs'
 */
import { chromium, devices } from 'playwright';

const BASE = process.env.BASE || 'http://127.0.0.1:8080';
const EMAIL = process.env.EMAIL;
const MDP = process.env.MDP;
if (!EMAIL || !MDP) {
  console.error('EMAIL et MDP sont requis.');
  process.exit(2);
}

const PROFILS = [
  { nom: 'iPhone 13', ...devices['iPhone 13'] },
  { nom: 'Pixel 5', ...devices['Pixel 5'] },
  // Le plus petit écran encore en service : iPhone SE de première génération.
  { nom: '320 px', viewport: { width: 320, height: 640 }, isMobile: true,
    hasTouch: true, deviceScaleFactor: 2,
    userAgent: 'Mozilla/5.0 (Linux; Android 10) AppleWebKit/537.36 Mobile Safari/537.36' },
];

const PAGES = ['/', '/parc', '/scan', '/clients', '/documents', '/lots',
               '/operations/nouvelle', '/administration',
               '/administration/referentiels', '/administration/utilisateurs',
               '/administration/parametres', '/compte'];

const nav = await chromium.launch();
const echecs = [];

for (const profil of PROFILS) {
  const ctx = await nav.newContext({ ...profil, locale: 'fr-FR' });
  const page = await ctx.newPage();
  await page.goto(`${BASE}/connexion`, { waitUntil: 'networkidle' });
  await page.fill('#champ-email', EMAIL);
  await page.fill('#champ-mot_de_passe', MDP);
  await Promise.all([page.waitForURL('**/'), page.click('button[type=submit]')]);

  await page.goto(`${BASE}/parc`, { waitUntil: 'networkidle' });
  const fiche = await page.locator('a.lien-ligne').first().getAttribute('href').catch(() => null);
  const liste = fiche ? [...PAGES, fiche] : PAGES;

  for (const chemin of liste) {
    await page.goto(BASE + chemin, { waitUntil: 'networkidle' });
    await page.waitForTimeout(150);
    const r = await page.evaluate(() => {
      const de = document.documentElement;
      const coupables = [];
      for (const el of document.querySelectorAll('body *')) {
        const b = el.getBoundingClientRect();
        if (b.width > 0 && b.right > de.clientWidth + 1) {
          const nom = (e) => e.tagName.toLowerCase() +
            (e.id ? '#' + e.id : '') +
            (e.className ? '.' + String(e.className).trim().split(/\s+/).slice(0, 2).join('.') : '');
          coupables.push(`${nom(el)} [${Math.round(b.left)}..${Math.round(b.right)}]`);
        }
      }
      return { debordement: de.scrollWidth - de.clientWidth,
               largeur: de.clientWidth,
               coupables: [...new Set(coupables)].slice(0, 5) };
    });
    if (r.debordement > 1) {
      echecs.push(`${profil.nom} ${chemin} : ${r.debordement} px de trop ` +
                  `(écran ${r.largeur} px)\n      ${r.coupables.join('\n      ')}`);
    }
  }
  await ctx.close();
  console.log(`  ${profil.nom} : ${liste.length} pages vérifiées`);
}
await nav.close();

if (echecs.length) {
  console.error('\nDébordement horizontal :\n  ' + echecs.join('\n  '));
  process.exit(1);
}
console.log('\nAucun débordement horizontal.');
