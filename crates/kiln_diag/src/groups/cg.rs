// Warning groups named by `cg_*` ids (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

AtomicAlignment = "atomic-alignment" [];
// WP-448: kiln-only remark group (`-Rauto-var-init`).
AutoVarInit = "auto-var-init" [];
// clang 19 only (WP-1795).
HigherPrecisionFp = "higher-precision-fp" [];
Psabi = "psabi" [];
SyncAlignment = "sync-alignment" [];
