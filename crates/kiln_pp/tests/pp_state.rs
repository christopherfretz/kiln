//! The preprocessor-state record (spec/pp.md "Preprocessor-state
//! record", spec/modules.md §11.13): a header's state written, read
//! back and installed into a fresh preprocessor gives the same `-E` and
//! `-dM -E` output and the same diagnostics as including the header
//! ([cpp.include]/2, [cpp.replace]/2, [cpp.pragma]).

mod common;

use common::PREDEFINES;
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std};
use kiln_pp::state::{PpRecord, StateHooks};
use kiln_pp::{MemoryResolver, PpOptions, Preprocessor, PrintOptions, print_macros, print_preprocessed};
use kiln_source::SourceManager;

fn opts(predefines: &str) -> PpOptions {
    let mut o = PpOptions::new(Std::Cxx26);
    o.predefines = predefines.to_owned();
    o.reproducible_time = Some(("Jan  1 2000".to_owned(), "00:00:00".to_owned()));
    o
}

/// The record of header `h` (read as the main file, as `-emit-pch` does).
fn record(h: &str) -> Vec<u8> {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("h.h", h.to_owned()).unwrap();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut o = opts(PREDEFINES);
    o.main_is_header = true;
    let mut pp = Preprocessor::new(&sm, &engine, &interner, o);
    pp.set_state_hooks(StateHooks { baseline: true, ..StateHooks::default() });
    pp.enter_main_file(main);
    let mut sink = String::new();
    print_preprocessed(&mut pp, PrintOptions::default(), &mut sink);
    let hooks = pp.take_state_hooks().unwrap();
    let base = hooks.taken_baseline.expect("baseline taken");
    let st = pp.state_snapshot(&base);
    st.encode(&sm, &interner, main).expect("the record encodes")
}

/// `-E -P` (or `-dM -E` with `dm`) of `rest`, and its diagnostics: with
/// `header` included first, or with the record of `header` installed.
fn run(header: &str, rest: &str, load: bool, dm: bool) -> (String, Vec<String>) {
    let bytes = record(header);
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.cpp", rest.to_owned()).unwrap();
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let pre =
        if load { PREDEFINES.to_owned() } else { format!("{PREDEFINES}\n#include \"h.h\"\n") };
    let mut pp = Preprocessor::new(&sm, &engine, &interner, opts(&pre));
    pp.set_include_resolver(Box::new(MemoryResolver::new().with_header("h.h", header)));
    if load {
        let rec = PpRecord::decode(&bytes).expect("the record decodes");
        let st = rec.materialize(&sm, &interner).expect("the record materializes");
        pp.set_state_hooks(StateHooks { install: Some((0, st)), ..StateHooks::default() });
    }
    pp.enter_main_file(main);
    let mut text = String::new();
    if dm {
        print_macros(&mut pp, &mut text);
    } else {
        print_preprocessed(&mut pp, PrintOptions::default(), &mut text);
    }
    drop(pp);
    let diags = sink
        .take()
        .iter()
        .map(|d| {
            let at = sm
                .presumed_line_col(d.span.lo)
                .map_or_else(String::new, |p| format!("{}:{}:{}: ", p.file_name, p.line, p.col));
            format!("{at}{}: {}", d.level.text_name(), d.message)
        })
        // What the header itself prints and diagnoses is the producer's.
        .filter(|d: &String| !d.starts_with("h.h:"))
        .collect();
    (text, diags)
}

/// The non-empty lines of `s` after the first `skip`.
fn lines(s: &str, skip: usize) -> Vec<&str> {
    s.lines().filter(|l| !l.trim().is_empty()).skip(skip).collect()
}

/// Asserts loading the record of `header` before `rest` matches
/// including it, for `-E` and `-dM -E`, and returns the `-E` run.
#[track_caller]
fn same(header: &str, rest: &str) -> (String, Vec<String>) {
    let replay = run(header, rest, false, false);
    let load = run(header, rest, true, false);
    let own = lines(&run(header, "", false, false).0, 0).len();
    assert_eq!(lines(&replay.0, own), lines(&load.0, 0), "-E of\n{rest}\nafter\n{header}");
    assert_eq!(replay.1, load.1, "diagnostics of\n{rest}\nafter\n{header}");
    let replay_dm = run(header, rest, false, true);
    let load_dm = run(header, rest, true, true);
    assert_eq!(replay_dm, load_dm, "-dM -E of\n{rest}\nafter\n{header}");
    replay
}

#[test]
fn macros_and_undefs() {
    let (text, _) = same(
        "#define OBJ 1 + 2\n#define FN(a, ...) a(__VA_ARGS__) #a\n#define NAMED(x, rest...) x rest\n\
         #undef __GNUC__\n#define __clang_major__ 99\n#define EMPTY\n#define PASTE(a,b) a ## b\n\
         #define STR(x) u8\"s\" L'c' R\"(raw)\" 0x1p3 x\n",
        "OBJ FN(f, 1, 2) NAMED(1, 2, 3) PASTE(x, y) STR(z) EMPTY __clang_major__\n\
         #ifdef __GNUC__\ngnuc\n#endif\n",
    );
    assert!(text.contains("1 + 2"), "{text}");
    assert!(!text.contains("gnuc"), "{text}");
}

#[test]
fn redefinition_diagnostic_points_into_the_header() {
    let (_, diags) = same("#define A 1\n#define B(x) x\n", "#define A 2\n#define B(y) y\nA B(3)\n");
    assert_eq!(diags.len(), 2, "{diags:?}");
}

#[test]
fn push_pop_macro_and_poison() {
    let (text, diags) = same(
        "#define M 1\n#pragma push_macro(\"M\")\n#undef M\n#define M 2\n\
         #pragma push_macro(\"U\")\n#define U 3\n#pragma GCC poison bad\n",
        "M U\n#pragma pop_macro(\"M\")\n#pragma pop_macro(\"U\")\nM U\nbad\n",
    );
    assert!(text.contains("2 3"), "{text}");
    assert!(text.contains("1 U"), "{text}");
    assert_eq!(diags.len(), 1, "the poisoned identifier: {diags:?}");
}

#[test]
fn counter() {
    let (text, _) = same("#if __COUNTER__ + __COUNTER__\n#endif\n", "__COUNTER__ __COUNTER__\n");
    assert!(text.contains("2 3"), "{text}");
}

#[test]
fn diagnostic_pragma_stack() {
    let (_, diags) = same(
        "#pragma clang diagnostic push\n#pragma clang diagnostic ignored \"-W#warnings\"\n\
         #pragma GCC diagnostic warning \"-Wunknown-pragmas\"\n",
        "#warning silenced\n#pragma clang diagnostic pop\n#warning loud\n\
         #pragma clang diagnostic pop\n",
    );
    assert!(diags.iter().any(|d| d.contains("loud")), "{diags:?}");
    assert!(!diags.iter().any(|d| d.contains("silenced")), "{diags:?}");
}

#[test]
fn record_is_total() {
    // Every prefix and every single-byte change of a record either
    // decodes or is an error, never a panic.
    let bytes = record("#define F(a) a + 1\n#pragma push_macro(\"F\")\n#pragma GCC poison p\n");
    for n in 0..bytes.len() {
        let _ = PpRecord::decode(&bytes[..n]);
    }
    for i in 0..bytes.len() {
        let mut b = bytes.clone();
        b[i] ^= 0x5a;
        if let Ok(r) = PpRecord::decode(&b) {
            let interner = Interner::new();
            let sm = SourceManager::new();
            let _ = r.materialize(&sm, &interner);
        }
    }
}
