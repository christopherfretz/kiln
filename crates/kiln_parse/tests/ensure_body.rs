//! `SemaBridge::ensure_body` instantiating a definition from inside a
//! constant evaluation (spec/sema-templates.md §4.11, R-OVEN-1;
//! [temp.inst]/8: a definition needed for constant evaluation is
//! instantiated).
//!
//! Every expression sema builds marks what it names, so the Oven reaches
//! an uninstantiated member only through a path sema did not mark (a
//! virtual call, before WP-067b's construction lands). These tests build
//! the call to `U<long>::f` directly with the AST builder, bypassing
//! marking, and evaluate it.

use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{CastKind, ExprFlags, NonOdrUse, ValueCategory};
use kiln_ast::{DeclId, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::Std;
use kiln_lex::literal::{EncodingParams, LiteralOptions};
use kiln_parse::{Parser, TokenSource};
use kiln_pp::{PpOptions, Preprocessor};
use kiln_sema::target::{Arch, Target};
use kiln_sema::templ::report::ReportFormat;
use kiln_sema::{EvalContext, EvalKind, Sema};
use kiln_source::symbol::Interner;
use kiln_source::{Loc, SourceManager};

/// Parses `src`, then runs `check` on the analysed TU's `Sema` and the
/// diagnostics emitted so far.
fn with_tu(src: &str, check: impl FnOnce(&mut Sema<'_, '_>, &MemorySink)) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let diags = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::X86_64);
    let mut pp = Preprocessor::new(&sm, &diags, &interner, PpOptions::new(Std::Cxx26));
    pp.enter_main_file(file);
    let ast = kiln_ast::Ast::new(
        Arc::clone(&interner),
        kiln_ast::types::TypeContext::new(target.types.clone()),
    );
    let enc = EncodingParams {
        char_signed: target.types.char_signed,
        wchar_bits: 32,
        wchar_signed: target.types.wchar_signed,
    };
    let mut s = Sema::for_target(ast, &diags, target);
    s.inst.report.format = ReportFormat::Json;
    let mut p = Parser::new(s, TokenSource::new(pp, &diags), LiteralOptions::new(Std::Cxx26, enc));
    p.parse_translation_unit();
    let (mut s, _) = p.finish();
    check(&mut s, &sink);
}

/// The member function `name` of the class template specialization
/// (the only instantiated one of that name).
fn member_of_spec(s: &Sema<'_, '_>, name: &str) -> DeclId {
    let sym = s.ast.symbols().intern(name);
    s.ast
        .decls
        .ids()
        .find(|&d| {
            s.ast.decls.get(d).kind() == DeclKind::CXXMethod
                && s.ast.decls.get(d).name().as_ident() == Some(sym)
                && s.ast.instantiated_from(d).is_some()
        })
        .expect("the instantiated member declaration")
}

/// `f()` built without marking `f` referenced: a `CallExpr` of the
/// decayed `DeclRefExpr`.
fn unmarked_call(s: &mut Sema<'_, '_>, f: DeclId) -> ExprId {
    let fty = s.ast.decls.get(f).ty().expect("a function type");
    let ret = s.ast.types.proto(fty).expect("a prototype").ret();
    let pty = s.ast.types.pointer_to(fty);
    let mut b = s.builder();
    let callee = b.decl_ref(f, fty, Loc::INVALID, None, NonOdrUse::None, ExprFlags::NONE);
    let decayed =
        b.implicit_cast(CastKind::FunctionToPointerDecay, callee, pty, ValueCategory::PRValue);
    b.call(decayed, &[], ret, ValueCategory::PRValue, Loc::INVALID, Loc::INVALID, ExprFlags::NONE)
}

fn ctx() -> EvalContext {
    EvalContext::new(EvalKind::StaticAssert, Loc::INVALID)
}

const U: &str = "template <class T> struct U {\n\
                   static constexpr int f() { return sizeof(T); }\n\
                 };\n\
                 constexpr unsigned long n = sizeof(U<long>);\n";

/// [temp.inst]/8: the unmarked member's definition is instantiated when
/// the evaluation needs it, once (cached: the second evaluation finds
/// the body), and recorded in the instantiation report.
#[test]
fn instantiates_unmarked_member_once() {
    with_tu(U, |s, sink| {
        let f = member_of_spec(s, "f");
        assert!(s.ast.definition(f).is_none(), "f is not instantiated by declaring U<long>");
        let call = unmarked_call(s, f);
        assert_eq!(s.eval_int(call, ctx()).ok(), Some(8));
        let def = s.ast.definition(f).expect("instantiated by ensure_body");
        let again = unmarked_call(s, f);
        assert_eq!(s.eval_int(again, ctx()).ok(), Some(8));
        assert_eq!(s.ast.definition(f), Some(def), "not instantiated twice");
        let report = s.instantiation_report("t.cpp");
        assert!(
            report.contains(r#"{"template":"U::f","kind":"member","loc":"t.cpp:2:22","count":1,"#),
            "{report}"
        );
        let d = sink.take();
        assert!(d.is_empty(), "{d:?}");
    });
}

/// A speculative fold (`try_fold`) never instantiates: the call is
/// simply not a constant.
#[test]
fn speculative_fold_does_not_instantiate() {
    with_tu(U, |s, _| {
        let f = member_of_spec(s, "f");
        let call = unmarked_call(s, f);
        assert!(s.fold_int(call).is_none());
        assert!(s.ast.definition(f).is_none());
    });
}

/// R-OVEN-1: the instantiated body's `static_assert` evaluates nested,
/// through the Oven lent back to sema, and its call to another constexpr
/// function works; the outer evaluation then completes.
#[test]
fn nested_evaluation_in_instantiated_body() {
    let src = "constexpr int two() { return 2; }\n\
               template <class T> struct U {\n\
                 static constexpr int f() { static_assert(two() * sizeof(T) == 16); return two(); }\n\
               };\n\
               constexpr unsigned long n = sizeof(U<long>);\n";
    with_tu(src, |s, sink| {
        let f = member_of_spec(s, "f");
        let call = unmarked_call(s, f);
        assert_eq!(s.eval_int(call, ctx()).ok(), Some(2));
        let d = sink.take();
        assert!(d.is_empty(), "{d:?}");
        assert!(s.oven().stats().records().len() >= 2, "the nested evaluation is recorded");
    });
}

/// An error in the instantiated definition is reported (with the
/// instantiation context) and, as in clang, the definition still exists:
/// a failed `static_assert` does not make it invalid.
#[test]
fn instantiation_errors_are_reported() {
    let src = "template <class T> struct U {\n\
                 static constexpr int f() { static_assert(sizeof(T) == 1); return 0; }\n\
               };\n\
               constexpr unsigned long n = sizeof(U<long>);\n";
    with_tu(src, |s, sink| {
        let f = member_of_spec(s, "f");
        let call = unmarked_call(s, f);
        assert_eq!(s.eval_int(call, ctx()).ok(), Some(0));
        let d = sink.take();
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].message, "static assertion failed due to requirement 'sizeof(long) == 1'");
        let notes: Vec<&str> = d[0].notes.iter().map(|n| n.message.as_str()).collect();
        assert!(
            notes
                .iter()
                .any(|n| n
                    .contains("in instantiation of member function 'U<long>::f' requested here")),
            "{notes:?}"
        );
    });
}

/// A member marked while its pattern had no definition (`g` names
/// `S<int>::f` before `S<T>::f` is defined) is queued and instantiated
/// at end of TU ([temp.point]/7), not dropped. (Within the TU, constant
/// evaluation still finds it undefined, as clang 19 does: the
/// `templates/class/temp.inst-constexpr-defined-later.cpp` test.)
#[test]
fn marked_before_pattern_definition_is_instantiated_at_end_of_tu() {
    let src = "template <class T> struct S { static constexpr int f(); };\n\
               constexpr int g() { return S<int>::f(); }\n\
               template <class T> constexpr int S<T>::f() { return 42; }\n";
    with_tu(src, |s, _| {
        let f = member_of_spec(s, "f");
        assert!(s.ast.definition(f).is_some());
        let call = unmarked_call(s, f);
        assert_eq!(s.eval_int(call, ctx()).ok(), Some(42));
    });
}
