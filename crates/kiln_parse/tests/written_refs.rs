//! The written-reference table (spec/ast.md §18): off by default, it
//! records nothing and reports no memory; on, it records each written
//! name once and dumps it.

use std::sync::Arc;

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

const SRC: &str = "namespace n { int f(int); int f(long); int v; }\n\
                   struct S { int m; };\n\
                   int g(S s) { return n::f(n::v) + s.m; }\n";

/// The dump of `SRC` with recording `on`, and whether the table
/// reported a memory row.
fn run(on: bool) -> (usize, String, bool) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", SRC.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions { written_refs: on, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let t = &tu.ast.written_refs;
    assert_eq!(t.is_on(), on);
    let row = tu.ast.memory().rows.iter().any(|r| r.name == "written_refs");
    (t.len(), t.dump(&tu.ast, &sm, Some(file)), row)
}

#[test]
fn off_records_nothing() {
    let (n, dump, row) = run(false);
    assert_eq!(n, 0);
    assert!(dump.is_empty());
    assert!(!row, "no memory row when off");
}

#[test]
fn on_records_each_name_once() {
    let (n, dump, row) = run(true);
    assert!(row);
    let want = "3:21-3:22  nns  'n' -> n@1:11 in=g@3:5\n\
                3:24-3:25  expr  'f' -> n::f@1:19 q=3:21 in=g@3:5\n\
                3:26-3:27  nns  'n' -> n@1:11 in=g@3:5\n\
                3:29-3:30  expr  'v' -> n::v@1:44 q=3:26 in=g@3:5\n\
                3:34-3:35  expr  's' -> s@3:9 in=g@3:5\n\
                3:36-3:37  member  'm' -> S::m@2:16 in=g@3:5\n";
    assert_eq!(dump, want);
    assert_eq!(n, 6);
}
