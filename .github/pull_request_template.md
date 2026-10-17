## Summary

<!-- What does this change and why? -->

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Refactoring
- [ ] Documentation
- [ ] CI / infrastructure

## Checklist

- [ ] New route: registered in `INVENTAIRE` with its minimal role
- [ ] Modified SQLx query: `cargo sqlx prepare --workspace -- --all-targets` re-run and `.sqlx` committed
- [ ] New business table: `tenant_id` column and `select appliquer_rls();` at the end of the migration
- [ ] New variant of `Statut`, `Role`, `TypeEvenement` or `TypeDocument`: `TOUS`, `cle_i18n()` and `crates/web/src/i18n/fr.rs` updated
- [ ] User-visible text goes through `t!("key")`, no literal in templates
- [ ] No inline JavaScript (strict CSP)
- [ ] New static asset: in `assets/`, embedded in `crates/web/src/statique.rs`, registered in `crates/web/build.rs` if needed
- [ ] `./dev.sh cargo fmt --all --check` passes
- [ ] `./dev.sh cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `./dev.sh cargo test --workspace` passes
- [ ] UI change: mobile audit run (`node outils/audit-mobile.mjs`)
