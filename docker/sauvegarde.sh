#!/bin/sh
# Sauvegarde quotidienne avec rotation.
#
# Une sauvegarde jamais restaurée n'est pas une sauvegarde : la procédure
# est décrite dans docs/exploitation.md et rejouée par l'intégration
# continue. Ici, on se contente de deux garde-fous qui attrapent les deux
# pannes silencieuses les plus courantes : dumper une base pas encore
# migrée, et dumper une base vide sans s'en apercevoir.
set -eu

DOSSIER=/sauvegardes
TABLE_TEMOIN=${TABLE_TEMOIN:-evenement_fut}
mkdir -p "$DOSSIER/quotidien" "$DOSSIER/hebdomadaire" "$DOSSIER/mensuel"

journal() { echo "[$(date -Is)] $*"; }

attendre_le_schema() {
  # Au premier démarrage, l'application joue ses migrations en parallèle.
  # Dumper avant qu'elles soient passées produit une archive vide.
  i=0
  while [ "$i" -lt 120 ]; do
    if psql -tAqc "select to_regclass('public.${TABLE_TEMOIN}') is not null" 2>/dev/null \
         | grep -q '^t$'; then
      return 0
    fi
    i=$((i + 1))
    sleep 5
  done
  journal "ABANDON : la table ${TABLE_TEMOIN} n'existe toujours pas après 10 minutes"
  return 1
}

sauvegarder() {
  horodatage=$(date +%Y-%m-%d_%H%M)
  fichier="$DOSSIER/quotidien/webqr_${horodatage}.dump"
  journal "sauvegarde vers $fichier"
  pg_dump --format=custom --compress=6 --file="$fichier.partiel"

  # Garde-fou : une archive qui ne contient pas la table témoin est une
  # archive inutilisable. Mieux vaut le savoir maintenant.
  if ! pg_restore --list "$fichier.partiel" | grep -q "TABLE DATA public ${TABLE_TEMOIN}"; then
    journal "ÉCHEC : l'archive ne contient pas les données de ${TABLE_TEMOIN}"
    rm -f "$fichier.partiel"
    return 1
  fi
  mv "$fichier.partiel" "$fichier"
  journal "archive validée ($(du -h "$fichier" | cut -f1))"

  [ "$(date +%u)" = "1" ] && cp "$fichier" "$DOSSIER/hebdomadaire/"
  [ "$(date +%d)" = "01" ] && cp "$fichier" "$DOSSIER/mensuel/"

  purger "$DOSSIER/quotidien"    "${RETENTION_QUOTIDIENNE:-7}"
  purger "$DOSSIER/hebdomadaire" "${RETENTION_HEBDOMADAIRE:-4}"
  purger "$DOSSIER/mensuel"      "${RETENTION_MENSUELLE:-6}"
  journal "terminé ($(ls -1 "$DOSSIER/quotidien" | wc -l) quotidiennes conservées)"
}

purger() {
  dossier="$1"; garder="$2"
  ls -1t "$dossier" 2>/dev/null | tail -n "+$((garder + 1))" | while read -r vieux; do
    rm -f "$dossier/$vieux"
  done
}

attendre_le_schema || exit 1
sauvegarder || journal "échec de la sauvegarde initiale"

# Une sauvegarde par jour à 03h15.
while true; do
  maintenant=$(date +%s)
  prochain=$(date -d "tomorrow 03:15" +%s 2>/dev/null || echo $((maintenant + 86400)))
  sleep $((prochain - maintenant))
  sauvegarder || journal "ÉCHEC de la sauvegarde"
done
