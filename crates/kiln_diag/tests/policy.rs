//! The `-W*` policy matrix (`spec/diag.md` sections 4 and 5). Expected
//! levels were taken from clang 19 on the equivalent preprocessor input.

mod common;

use common::{source, text};
use kiln_diag::{DiagEngine, DiagId, DiagOptions, MemorySink, Severity};

const IDS: [DiagId; 7] = [
    DiagId::pp_warning_directive, // -W#warnings, on
    DiagId::pp_macro_redefined,   // ExtWarn, -Wmacro-redefined
    DiagId::pp_pragma_message,    // -W#pragma-messages, never -Werror'd
    DiagId::pp_unknown_pragma,    // off, -Wunknown-pragmas (in -Wall, -Wpragmas)
    DiagId::pp_macro_unused,      // off, -Wunused-macros
    DiagId::pp_expected_eol,      // ExtWarn, -Wextra-tokens
    DiagId::pp_endif_without_if,  // error
];

fn levels(args: &str) -> String {
    let (sm, _) = source("p.c", "");
    let opts = DiagOptions::from_args(args.split_whitespace());
    let engine = DiagEngine::new(&sm, &opts, MemorySink::new());
    IDS.iter()
        .map(|&id| match engine.severity(id) {
            Severity::Ignored => '-',
            Severity::Remark => 'R',
            Severity::Warning => 'W',
            Severity::Error => 'E',
            Severity::Fatal => 'F',
        })
        .collect()
}

// spec: diag.md §5.2 (severity computation), §4 (flags)
#[test]
fn warning_flag_matrix() {
    let table = [
        ("", "WWW--WE"),
        ("-w", "------E"),
        ("-Werror", "EEW--EE"),
        ("-WX", "EEW--EE"),
        ("-Werror -w", "------E"),
        ("-w -Werror", "------E"),
        ("-Wno-#warnings", "-WW--WE"),
        ("-Werror=#warnings", "EWW--WE"),
        ("-Werror -Wno-error=macro-redefined", "EWW--EE"),
        ("-Wall", "WWWW-WE"),
        ("-Wall -Werror", "EEWE-EE"),
        ("-Wmost", "WWWW-WE"),
        ("-Wunused-macros", "WWW-WWE"),
        ("-Weverything", "WWWWWWE"),
        ("-Weverything -Wno-unused-macros", "WWWW-WE"),
        ("-Weverything -w", "------E"),
        ("-Werror=unused-macros", "WWW-EWE"),
        ("-Wno-error=unused-macros -Werror", "EEW--EE"),
        ("-Werror=macro-redefined -Wno-macro-redefined", "W-W--WE"),
        ("-Wno-macro-redefined -Werror=macro-redefined", "WEW--WE"),
        ("-Werror=macro-redefined -Wmacro-redefined", "WEW--WE"),
        ("-pedantic-errors", "WEW--EE"),
        // -Wno-error= is not a user mapping, so -pedantic-errors still applies.
        ("-pedantic-errors -Wno-error=extra-tokens", "WEW--EE"),
        ("-Wfatal-errors", "WWW--WF"),
        ("-Werror -Wfatal-errors", "FFW--FF"),
        ("-Werror -Wfatal-errors -Wno-fatal-errors=#warnings", "EFW--FF"),
        ("-Werror=#pragma-messages", "WWE--WE"),
        ("-Werror -Wpragmas", "EEWE-EE"),
        ("-Wall -Wno-pragmas", "WWW--WE"),
        ("-Wextra-tokens -Wno-extra-tokens", "WWW---E"),
        ("-Wno-everything", "------E"),
        ("-Wsystem-headers -Wno-unknown-warning-option -Wbogus", "WWW--WE"),
    ];
    let mut failures = Vec::new();
    for (args, want) in table {
        let got = levels(args);
        if got != want {
            failures.push(format!("{args:?}: got {got}, want {want}"));
        }
    }
    assert!(failures.is_empty(), "policy mismatches:\n{}", failures.join("\n"));
}

// spec: diag.md §5.3 (diagnostics about the options themselves)
#[test]
fn unknown_options() {
    let (sm, _) = source("p.c", "");
    let run = |args: &[&str]| text(&sm, args, |_| {});
    // Deviation: suggestions come from kiln's groups only. clang 19 ties
    // among its larger table here and suggests nothing.
    assert_eq!(
        run(&["-Wbogus"]),
        "warning: unknown warning option '-Wbogus'; did you mean '-Wmost'? [-Wunknown-warning-option]\n1 warning generated.\n"
    );
    assert_eq!(
        run(&["-Wno-macro-redefine"]),
        "warning: unknown warning option '-Wno-macro-redefine'; did you mean '-Wno-macro-redefined'? [-Wunknown-warning-option]\n1 warning generated.\n"
    );
    assert_eq!(
        run(&["-Werrorbogus"]),
        "warning: unknown -Werror warning specifier: '-Werrorbogus' [-Wunknown-warning-option]\n1 warning generated.\n"
    );
    assert_eq!(
        run(&["-Werror=bogus"]),
        "warning: unknown warning option '-Werror=bogus'; did you mean '-Werror=most'? [-Wunknown-warning-option]\n1 warning generated.\n"
    );
    assert_eq!(
        run(&["-Rbogus"]),
        "warning: unknown remark option '-Rbogus' [-Wunknown-warning-option]\n1 warning generated.\n"
    );
    // Reported after every mapping is applied, so a later
    // -Wno-unknown-warning-option silences an earlier unknown option.
    assert_eq!(run(&["-Wbogus", "-Wno-unknown-warning-option"]), "");
    // Names clang knows but kiln does not emit yet are accepted silently.
    assert_eq!(run(&["-Wshadow", "-Wno-error=unused-variable", "-Wextra"]), "");
    // -Werror makes the unknown-option warning an error, like clang.
    assert_eq!(
        run(&["-Werror", "-Wbogusbogus"]),
        "error: unknown warning option '-Wbogusbogus' [-Werror,-Wunknown-warning-option]\n1 error generated.\n"
    );
}

// spec: diag.md §4 (bad option values)
#[test]
fn bad_option_values() {
    let (sm, _) = source("p.c", "");
    let run = |args: &[&str]| text(&sm, args, |_| {});
    assert_eq!(
        run(&["-ferror-limit=abc"]),
        "error: invalid integral value 'abc' in '-ferror-limit abc'\n1 error generated.\n"
    );
    assert_eq!(
        run(&["-ftabstop=0"]),
        "warning: ignoring invalid -ftabstop value '0', using default value 8\n1 warning generated.\n"
    );
    assert_eq!(
        run(&["-fdiagnostics-format=xml"]),
        "error: invalid value 'xml' in '-fdiagnostics-format xml'\n1 error generated.\n"
    );
    let mut o = DiagOptions::default();
    assert!(!o.parse_arg("-Wl,--foo"));
    assert!(o.parse_arg("-fno-color-diagnostics") && o.parse_arg("-ferror-limit=0"));
    assert_eq!(o.error_limit, 0);
}
