#!/usr/bin/env node
/**
 * Vérifie qu'une planche d'étiquettes produite par l'application est bien
 * décodable — par le décodeur zxing **embarqué dans le binaire**, c'est-à-
 * dire exactement celui que rencontrera un opérateur.
 *
 * Cela couvre la moitié numérique du critère « une planche imprimée est
 * scannable ». L'autre moitié — l'imprimante, l'encre et le support —
 * demande du papier et reste un contrôle manuel.
 *
 *   node outils/verifier-etiquettes.mjs planche.html
 *
 * où planche.html est la page enregistrée depuis
 *   /administration/etiquettes/planche?de=1&a=24
 */
import { readFileSync } from 'node:fs';
import { runInThisContext } from 'node:vm';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const racine = join(dirname(fileURLToPath(import.meta.url)), '..');
const fichier = process.argv[2];
if (!fichier) {
  console.error('usage : node outils/verifier-etiquettes.mjs <planche.html>');
  process.exit(2);
}

const html = readFileSync(fichier, 'utf8');
const svgs = [...html.matchAll(/<svg[\s\S]*?<\/svg>/g)].map((m) => m[0]);
const attendus = [...html.matchAll(/class="etiquette__code">([^<]+)</g)].map((m) => m[1]);
if (!svgs.length) {
  console.error('aucune étiquette trouvée dans la page');
  process.exit(1);
}

/** Rastérise le SVG émis par le crate `qrcode` (fond + un chemin de carrés). */
function rasteriser(svg) {
  const largeur = parseInt(svg.match(/width="(\d+)"/)[1], 10);
  const d = svg.match(/<path[^>]*d="([^"]*)"/)[1];
  const gris = new Uint8Array(largeur * largeur).fill(255);
  for (const m of d.matchAll(/M(\d+(?:\.\d+)?) (\d+(?:\.\d+)?)h(\d+(?:\.\d+)?)v(\d+(?:\.\d+)?)/g)) {
    const [x, y, w, h] = m.slice(1).map((v) => Math.round(parseFloat(v)));
    for (let yy = y; yy < Math.min(y + h, largeur); yy++)
      for (let xx = x; xx < Math.min(x + w, largeur); xx++) gris[yy * largeur + xx] = 0;
  }
  const rgba = new Uint8ClampedArray(largeur * largeur * 4);
  for (let i = 0; i < gris.length; i++) {
    rgba[i * 4] = rgba[i * 4 + 1] = rgba[i * 4 + 2] = gris[i];
    rgba[i * 4 + 3] = 255;
  }
  return { data: rgba, width: largeur, height: largeur };
}

runInThisContext(
  readFileSync(join(racine, 'assets/vendor/zxing-reader.js'), 'utf8') +
    '\n;globalThis.ZXingWASM = ZXingWASM;'
);
globalThis.ZXingWASM.prepareZXingModule({
  overrides: { wasmBinary: readFileSync(join(racine, 'assets/vendor/zxing_reader.wasm')) },
});

let echecs = 0;
for (const [i, svg] of svgs.entries()) {
  const resultats = await globalThis.ZXingWASM.readBarcodes(rasteriser(svg), {
    formats: ['QRCode'],
    tryHarder: true,
  });
  const lu = resultats[0]?.text;
  const attendu = attendus[i];
  const ok = lu === attendu;
  if (!ok) echecs++;
  console.log(
    `  ${ok ? '✓' : '✗'} ${attendu ?? '?'}` +
      (ok ? `   (${resultats[0].format}, correction ${resultats[0].eccLevel})`
          : `   lu : ${lu ?? 'RIEN'}`)
  );
}
console.log(`\n${svgs.length - echecs}/${svgs.length} étiquettes décodées.`);
process.exit(echecs ? 1 : 0);
