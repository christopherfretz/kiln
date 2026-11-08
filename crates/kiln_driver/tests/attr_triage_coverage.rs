//! WP-805 (spec/sema.md §4.5): the attribute triage table
//! (`kiln_sema::attr_triage`) against clang's `__has_attribute` and
//! `__has_cpp_attribute` tables, on both targets at clang 19 and 22 in
//! C++26:
//!
//! - every spelling clang knows is implemented or has a row whose
//!   spelling bit is set for that target and release (none falls through
//!   to the `unknown attribute` warning while clang accepts it);
//! - every spelling bit a row sets is one clang knows there (no row
//!   rejects or ignores a spelling clang would warn about).
//!
//! `__declspec` attributes are not triaged (kiln has no
//! `-fms-extensions` mode).
#![forbid(unsafe_code)]

use kiln_ast::attr::AttrSyntax;
use kiln_pp::{ClangCompat, ClangFeatures, FeatureQueries, FeatureTable, table_keys};
use kiln_sema::attr_triage::{self, CL, G, GS, GSL, MS, OMP, STD, Support};

const TARGETS: [(&str, bool); 2] =
    [("aarch64-unknown-linux-gnu", false), ("x86_64-unknown-linux-gnu", true)];

/// The name as `Sema::act_on_attribute` looks it up.
fn strip(name: &str) -> &str {
    kiln_ast::attr::normalize_attr_name(name)
}

/// The `__has_cpp_attribute` scope of a spelling bit.
fn scope_of(bit: u8) -> Option<&'static str> {
    match bit {
        GS => Some("gnu"),
        CL => Some("clang"),
        GSL => Some("gsl"),
        MS => Some("msvc"),
        OMP => Some("omp"),
        _ => None,
    }
}

fn features(triple: &str, compat: ClangCompat) -> ClangFeatures {
    ClangFeatures::new(triple, kiln_lex::Std::Cxx26, &[]).unwrap().with_compat(compat)
}

#[test]
fn every_clang_spelling_is_implemented_or_triaged() {
    let mut missing = Vec::new();
    for (triple, x86) in TARGETS {
        for compat in ClangCompat::ALL {
            let f = features(triple, compat);
            let c = compat.major();
            for n in table_keys(FeatureTable::Attribute) {
                if f.has_attribute(n) == 0 {
                    continue;
                }
                let s =
                    attr_triage::attribute_support(strip(n), AttrSyntax::Gnu, None, c, true, x86);
                if s == Support::Unknown {
                    missing.push(format!("{triple} @{c}: __attribute__(({n}))"));
                }
            }
            for k in table_keys(FeatureTable::CppAttribute) {
                let (scope, n) = match k.split_once("::") {
                    Some((s, n)) => (Some(s), n),
                    None => (None, k),
                };
                if f.has_cpp_attribute(scope, n) == 0 {
                    continue;
                }
                // P3475R2 removes `carries_dependency` from C++26: kiln
                // warns as for an unknown attribute (WP-743), while clang
                // still reports it.
                if scope.is_none() && strip(n) == "carries_dependency" {
                    continue;
                }
                let s = attr_triage::attribute_support(
                    strip(n),
                    AttrSyntax::Cxx11,
                    scope,
                    c,
                    true,
                    x86,
                );
                if s == Support::Unknown {
                    missing.push(format!("{triple} @{c}: [[{k}]]"));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "spellings clang knows that kiln neither implements nor triages: {missing:#?}"
    );
}

#[test]
fn every_row_spelling_is_known_to_clang() {
    let mut extra = Vec::new();
    for (triple, x86) in TARGETS {
        for compat in ClangCompat::ALL {
            let f = features(triple, compat);
            let c = compat.major();
            for r in attr_triage::TABLE {
                let sp = r.spellings_at(c, x86);
                for bit in [G, GS, CL, STD, GSL, MS, OMP] {
                    if sp & bit == 0 {
                        continue;
                    }
                    // Under its name, or one clang spells with a leading
                    // `__` only (`__clang_arm_builtin_alias`).
                    let known = [r.name.to_string(), format!("__{}", r.name)].iter().any(|n| {
                        if bit == G {
                            f.has_attribute(n) != 0
                        } else {
                            f.has_cpp_attribute(scope_of(bit), n) != 0
                        }
                    });
                    if !known {
                        extra.push(format!("{triple} @{c}: {} bit {bit}", r.name));
                    }
                }
            }
        }
    }
    assert!(extra.is_empty(), "row spellings clang does not know: {extra:#?}");
}
