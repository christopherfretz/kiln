//! `va_arg` of a class that is not trivially copyable ([cstdarg.syn]/1;
//! clang `BuildVAArgExpr`, `-Wnon-pod-varargs`; WP-2362): the expression
//! stays a `VAArgExpr` and carries `ExprDeps::ERROR` exactly when the
//! diagnostic is an error, so a consumer walking the body sees it is
//! erroneous; with the warning disabled or not an error, the program is
//! valid and the node is ordinary.

use std::sync::Arc;

use kiln_ast::expr::{ExprDeps, ExprKind};
use kiln_ast::{Ast, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

const SRC: &str = "struct N { N(const N &); int m; };\n\
                   struct P { int m; };\n\
                   int f(__builtin_va_list l) { return __builtin_va_arg(l, N).m; }\n\
                   int g(__builtin_va_list l) { return __builtin_va_arg(l, P).m; }\n";

fn parse(warnings: &[&str]) -> (Ast, u32) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", SRC.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let diag = DiagOptions {
        warnings: warnings.iter().map(|&w| w.to_owned()).collect(),
        ..DiagOptions::default()
    };
    let engine = DiagEngine::new(&sm, &diag, MemorySink::new());
    let target = Target::for_arch(Arch::X86_64);
    let opts = ParseOptions::default();
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    (tu.ast, engine.error_count())
}

/// Whether each `VAArgExpr` (in source order: `N`, then `P`) has the
/// error bit.
fn error_bits(ast: &Ast) -> Vec<bool> {
    (1..=ast.exprs.len())
        .map(|i| ExprId::new(u32::try_from(i).expect("expression index")))
        .filter(|&e| ast.exprs.get(e).kind() == ExprKind::VAArgExpr)
        .map(|e| ast.exprs.get(e).deps().contains(ExprDeps::ERROR))
        .collect()
}

#[test]
fn non_pod_va_arg_is_erroneous_while_an_error() {
    let (ast, errors) = parse(&[]);
    assert_eq!(errors, 1);
    assert_eq!(error_bits(&ast), [true, false]);
}

#[test]
fn non_pod_va_arg_is_ordinary_when_allowed() {
    for w in [&["no-non-pod-varargs"][..], &["no-error=non-pod-varargs"]] {
        let (ast, errors) = parse(w);
        assert_eq!(errors, 0, "{w:?}");
        assert_eq!(error_bits(&ast), [false, false], "{w:?}");
    }
}
