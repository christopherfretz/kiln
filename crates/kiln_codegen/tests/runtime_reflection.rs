//! Code generation of variables that hold reflections, decided by value
//! (P4101R1; spec/codegen.md "Consteval-only variables", WP-1328 land 1).
//! An immediate object has no storage; an object whose reflections are
//! all null is well-formed since P4101R1 but needs a run-time
//! `std::meta::info`, which codegen does not represent yet: a
//! non-`constexpr` one is the error "not supported yet", never a silent
//! miscompile or a reference to a missing symbol. kiln only: clang 19/22
//! lack `^^`.

use std::sync::Arc;

use kiln_abi::{Arch, Target};
use kiln_codegen::{CodegenOptions, emit};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

const PRELUDE: &str = "using info = decltype(^^int);\n\
                       struct S { info r; const char* name; int n; };\n";

/// Parses `PRELUDE` + `src` with reflection (no sema error allowed) and
/// lowers it: the IR, or codegen's error message.
fn lower(src: &str) -> Result<String, String> {
    let sm = SourceManager::new();
    let text = format!("{PRELUDE}{src}");
    let file = sm.add_virtual("t.cpp", text).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::AArch64);
    let mut tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        target.clone(),
        &ParseOptions { reflection: true, ..ParseOptions::default() },
        ConstexprLimits::default(),
    );
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "sema errors: {errors:?}");
    let opts = CodegenOptions { source_filename: "t.cpp".to_owned(), ..CodegenOptions::default() };
    emit(&mut tu.ast, &mut tu.oven, &target, &opts).map(|m| m.to_string()).map_err(|e| e.msg)
}

const UNSUPPORTED: &str =
    "code generation for a run-time 'std::meta::info' value is not supported yet";

fn assert_unsupported(src: &str) {
    match lower(src) {
        Err(msg) => assert!(msg.contains(UNSUPPORTED), "{src}: unexpected error {msg}"),
        Ok(_) => panic!("{src}: lowered a run-time std::meta::info"),
    }
}

#[test]
fn null_reflection_objects_are_not_supported_yet() {
    assert_unsupported("info g;\n");
    assert_unsupported("S g;\n");
    assert_unsupported("info* g;\n");
    assert_unsupported("int f() { info x; (void)x; return 0; }\n");
    assert_unsupported("int f() { static info x; (void)x; return 0; }\n");
    assert_unsupported("int f() { S s{{}, \"x\", 1}; return s.n; }\n");
}

#[test]
fn immediate_objects_get_no_storage() {
    let ir = lower(
        "constexpr info r = ^^int;\n\
         constexpr S gs{^^int, \"g\", 1};\n\
         extern constexpr S xs{^^char, \"x\", 2};\n\
         int f() { constexpr S ls{^^long, \"l\", 3}; constexpr int n = ls.n + xs.n; return n; }\n",
    )
    .expect("codegen");
    for name in ["@r ", "@gs ", "@xs ", "@_ZL2gs", "@_ZL1r"] {
        assert!(!ir.contains(name), "emitted {name}:\n{ir}");
    }
}

#[test]
fn constexpr_null_holders_fold_their_members() {
    // `gs` holds a null reflection: not an immediate object, so its
    // members may be read at run time; it still has no storage, and each
    // read folds.
    let ir = lower(
        "constexpr S gs{{}, \"g\", 1};\n\
         const char* name() { return gs.name; }\n\
         int num() { constexpr S ls{{}, \"l\", 2}; (void)ls; return ls.n + gs.n; }\n",
    )
    .expect("codegen");
    assert!(!ir.contains("@_ZL2gs"), "emitted gs:\n{ir}");
    assert!(ir.contains("c\"g\\00\""), "the folded string:\n{ir}");
}
