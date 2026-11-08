//! The splice fast path (spec/reflection.md §5): a splice whose operand
//! is already a frozen reflection reads it without an Oven run, and each
//! such read is counted in the Oven report's `splice_fast` total.

use std::fmt::Write as _;
use std::sync::Arc;

use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Parses `src` with `-freflection`; returns the fast-path count, the
/// number of Oven evaluations and the number of errors.
fn run(src: &str) -> (u64, u64, usize) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions { reflection: true, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let errors = sink.take().iter().filter(|d| d.level >= Level::Error).count();
    let t = tu.oven.stats().totals();
    (t.splice_fast, t.evaluations, errors)
}

#[test]
fn frozen_operands_skip_the_oven() {
    let mut src = String::from(
        "namespace N { int x = 2; }\nint v = 1;\n\
         constexpr decltype(^^int) rt = ^^int, rv = ^^v, rn = ^^N;\n",
    );
    for i in 0..1000 {
        writeln!(src, "using T{i} = [: rt :]; int f{i}() {{ return [: rv :] + [: rn :]::x; }}")
            .expect("write to a String");
    }
    let (fast, evaluations, errors) = run(&src);
    assert_eq!(errors, 0);
    assert_eq!(fast, 3000, "every splice reads its frozen operand");
    // Only the three initializers are evaluated.
    assert_eq!(evaluations, 3);
}

#[test]
fn reflect_expressions_and_nttps_are_frozen() {
    let src = "template <decltype(^^int) R> using A = [: R :];\n\
               using B = [: ^^int :];\n\
               A<^^long> a = 1;\n";
    let (fast, evaluations, errors) = run(src);
    assert_eq!(errors, 0);
    // `[: ^^int :]` and the substituted `[: R :]`.
    assert_eq!(fast, 2);
    // The splices add no evaluation to the same program without them
    // (whose one evaluation is the template argument `^^long`).
    let plain = "template <decltype(^^int) R> using A = long;\n\
                 using B = int;\n\
                 A<^^long> a = 1;\n";
    assert_eq!(run(plain), (0, evaluations, 0));
}

#[test]
fn splice_template_ids_read_frozen_operands() {
    // P2996 [temp.names]/1: `[: r :] < args >` in a type, expression and
    // scope position, dependent and not (WP-482).
    let src = "template <class T> struct Box { using type = T; };\n\
               template <class T> constexpr int var = 1;\n\
               template <class T> int g(T) { return 0; }\n\
               constexpr decltype(^^int) rb = ^^Box;\n\
               template <decltype(^^int) R> using A = typename [: R :]<int>;\n\
               using B = [: rb :]<long>;\n\
               A<^^Box> a;\n\
               template [: rb :]<char>::type c = 0;\n\
               int h() { return template [: ^^var :]<int> + template [: ^^g :]<int>(1); }\n";
    let (fast, evaluations, errors) = run(src);
    assert_eq!(errors, 0);
    // `[: rb :]` twice, `^^var`, `^^g` and the substituted `[: R :]`.
    assert_eq!(fast, 5);
    let plain = "template <class T> struct Box { using type = T; };\n\
                 template <class T> constexpr int var = 1;\n\
                 template <class T> int g(T) { return 0; }\n\
                 constexpr decltype(^^int) rb = ^^Box;\n\
                 template <decltype(^^int) R> using A = Box<int>;\n\
                 using B = Box<long>;\n\
                 A<^^Box> a;\n\
                 Box<char>::type c = 0;\n\
                 int h() { return var<int> + g<int>(1); }\n";
    assert_eq!(run(plain), (0, evaluations, 0));
}
