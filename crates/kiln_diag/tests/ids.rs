//! The id table and templates (`spec/diag.md` sections 2 and 3).

use kiln_diag::template::{check, render};
use kiln_diag::{DiagClass, DiagId, DiagKind, Group, Severity};

// spec: diag.md §2.2 (static placeholder/argument-count check)
#[test]
fn every_template_matches_its_arg_count() {
    for (i, &id) in DiagId::ALL.iter().enumerate() {
        assert_eq!(id as usize, i);
        assert!(check(id.template(), id.arg_count()), "{}", id.name());
        assert_eq!(DiagId::from_name(id.name()), Some(id));
        // Rendering with the right number of arguments leaves no braces
        // that were not literal.
        let args: Vec<String> = (0..id.arg_count()).map(|n| format!("<{n}>")).collect();
        let out = render(id.template(), &args);
        for n in 0..id.arg_count() {
            let used =
                out.contains(&format!("<{n}>")) || id.template().contains(&format!("{{{n}:"));
            assert!(used, "{}: argument {n} not rendered in {out:?}", id.name());
        }
    }
    assert_eq!(DiagId::from_name("no_such_id"), None);
}

// spec: diag.md §2.2
#[test]
fn check_rejects_mismatches() {
    assert!(check("plain", 0));
    assert!(check("{0} {1}", 2));
    assert!(check("{1} {0} {0}", 2));
    assert!(check("{{literal}} {0}", 1));
    assert!(check("{0:zero|one|{1} many}", 2));
    assert!(!check("{0}", 0)); // index out of range
    assert!(!check("{0}", 2)); // {1} unused
    assert!(!check("{1}", 2)); // {0} unused
    assert!(!check("{}", 0));
    assert!(!check("{0", 1));
    assert!(!check("0}", 1));
    assert!(!check("{x}", 1));
    assert!(!check("{0:a|{1:b|c}}", 2)); // nested select
    assert!(!check("{0:a|b", 1)); // unterminated select
}

// spec: diag.md §2.3 (rendering)
#[test]
fn render_templates() {
    let a = |v: &[&str]| v.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(render("'{0}' and '{1}'", &a(&["x", "y"])), "'x' and 'y'");
    assert_eq!(render("{{0}} {0}", &a(&["z"])), "{0} z");
    // A select picks alternative N by index (not a plural rule).
    let sel = "{0:zero|one|many} {1:no|yes}";
    assert_eq!(render(sel, &a(&["0", "1"])), "zero yes");
    assert_eq!(render(sel, &a(&["1", "0"])), "one no");
    // Out-of-range or non-numeric selectors pick the last alternative.
    assert_eq!(render(sel, &a(&["7", "x"])), "many yes");
    assert_eq!(render("{0:a|b{1}c}", &a(&["1", "-"])), "b-c");
    // Missing arguments render as nothing.
    assert_eq!(render("[{0}]", &[]), "[]");
}

// spec: diag.md §2.1, §3 (kinds, default severities, groups)
#[test]
fn kinds_and_groups() {
    for &id in DiagId::ALL {
        let k = id.kind();
        match k {
            DiagKind::Note => assert_eq!(id.class(), DiagClass::Note),
            DiagKind::Error | DiagKind::Fatal => {
                assert_eq!(id.class(), DiagClass::Error);
                assert!(id.group().is_none(), "{}: hard errors have no group", id.name());
            }
            _ => {}
        }
        if id.name().starts_with("note_") {
            assert_eq!(k, DiagKind::Note, "{}", id.name());
        }
        if let Some(g) = id.group() {
            assert!(g.members().any(|m| m == id), "{} not a member of {}", id.name(), g.name());
            assert_eq!(Group::from_name(g.name()), Some(g));
        }
    }
    assert_eq!(DiagId::pp_macro_unused.default_severity(), Severity::Ignored);
    assert_eq!(DiagId::pp_warning_directive.default_severity(), Severity::Warning);
    assert_eq!(DiagId::pp_endif_without_if.default_severity(), Severity::Error);
    assert_eq!(DiagId::diag_too_many_errors.default_severity(), Severity::Fatal);
    // Group nesting: -Wall reaches unknown-pragmas through -Wmost.
    let all = Group::from_name("all").unwrap();
    assert!(all.members().any(|m| m == DiagId::pp_unknown_pragma));
    assert!(!all.members().any(|m| m == DiagId::pp_macro_unused));
}
