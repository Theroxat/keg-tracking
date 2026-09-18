//! Calcule une empreinte des ressources statiques : chaque déploiement
//! invalide le cache des navigateurs sans intervention manuelle.
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn main() {
    let fichiers = [
        "../../assets/css/app.css",
        "../../assets/js/scan/scan.js",
        "../../assets/vendor/htmx.min.js",
    ];
    let mut h = DefaultHasher::new();
    for f in fichiers {
        println!("cargo:rerun-if-changed={f}");
        if let Ok(contenu) = std::fs::read(f) {
            contenu.hash(&mut h);
        }
    }
    println!("cargo:rustc-env=EMPREINTE_STATIQUE={:x}", h.finish());
}
