//! Contract assertions in codegen until WP-707 (spec/contracts.md §6):
//! skipped under the ignore semantic, `unsupported` under the others, so
//! a parsed contract is never silently dropped from the IR.

use std::sync::Arc;

use kiln_abi::{Arch, Target};
use kiln_ast::stmt::ContractSemantic;
use kiln_codegen::{CodegenOptions, emit};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Lowers `src` under `semantic`: the IR text or the codegen error.
fn lower(src: &str, semantic: ContractSemantic) -> Result<String, String> {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::AArch64);
    let opts = ParseOptions { contract_semantic: semantic, ..ParseOptions::default() };
    let mut tu =
        parse_tu(&sm, &interner, file, &engine, target.clone(), &opts, ConstexprLimits::default());
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let cg = CodegenOptions { source_filename: "t.cpp".to_owned(), ..CodegenOptions::default() };
    emit(&mut tu.ast, &mut tu.oven, &target, &cg).map(|m| m.to_string()).map_err(|e| e.msg)
}

const FUNCTION: &str = "int f(const int x) pre(x > 0) post(r: r > x) { return x + 1; }\n\
                        int g() { return f(1); }\n";
const DECLARED: &str = "int f(int x) pre(x > 0);\nint f(int x) { return x; }\n";
const ASSERT: &str = "int f(int x) { contract_assert(x > 0); return x; }\n";
const NONE: &str = "int f(int x) { return x; }\n";

#[test]
fn ignore_lowers_without_the_contracts() {
    for src in [FUNCTION, DECLARED, ASSERT] {
        let ir = lower(src, ContractSemantic::Ignore).expect("lowers under ignore");
        assert!(ir.contains("define"), "{ir}");
    }
}

#[test]
fn checking_semantics_are_unsupported() {
    for k in [ContractSemantic::Observe, ContractSemantic::Enforce, ContractSemantic::QuickEnforce]
    {
        for src in [FUNCTION, DECLARED, ASSERT] {
            let e = lower(src, k).expect_err("a checked contract is not dropped");
            assert_eq!(e, "unsupported: contract evaluation (WP-707)", "{k:?}: {src}");
        }
        // A program without contracts is unaffected.
        assert!(lower(NONE, k).is_ok());
    }
}
