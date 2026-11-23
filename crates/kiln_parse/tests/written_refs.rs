//! The written-reference table (spec/ast.md §18): off by default, it
//! records nothing and reports no memory; on, it records each written
//! name once and dumps it, and `enclosed_by` finds a declaration's type,
//! template, base and mem-init names (stage 2); the lookup provenance
//! reads through the API (§18.7).

use std::sync::Arc;

use kiln_ast::written_refs::RefRole;
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
    run_src(SRC, on, |_| {})
}

/// Parses `src` with recording `on`, hands the table to `check`, and
/// returns the entry count, the dump and whether a memory row exists.
fn run_src(
    src: &str,
    on: bool,
    check: impl FnOnce(&kiln_ast::written_refs::WrittenRefs),
) -> (usize, String, bool) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions { written_refs: on, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let t = &tu.ast.written_refs;
    assert_eq!(t.is_on(), on);
    check(t);
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
    let want = "3:7-3:8  type  'S' -> S@2:8 in=s@3:9\n\
                3:21-3:22  nns  'n' -> n@1:11 in=g@3:5\n\
                3:24-3:25  expr  'f' -> n::f@1:19 q=3:21 in=g@3:5\n\
                3:26-3:27  nns  'n' -> n@1:11 in=g@3:5\n\
                3:29-3:30  expr  'v' -> n::v@1:44 q=3:26 in=g@3:5\n\
                3:34-3:35  expr  's' -> s@3:9 in=g@3:5\n\
                3:36-3:37  member  'm' -> S::m@2:16 in=g@3:5\n";
    assert_eq!(dump, want);
    assert_eq!(n, 7);
}

const STAGE2: &str = "struct B { B(int); };\n\
                      template <class T> struct Box { T v; };\n\
                      struct D : B {\n\
                        int m;\n\
                        D() : B(1), m(2) {}\n\
                        Box<B> f(B* p);\n\
                      };\n";

/// `enclosed_by` on stage-2 entries: a class's base-specifier, a
/// constructor's mem-initializer-ids, a member function's return type and
/// template name, and a parameter's type are each enclosed by their own
/// declaration.
#[test]
fn enclosed_by_finds_stage2_names() {
    run_src(STAGE2, true, |t| {
        let first = |role| t.iter().find(|(_, r)| r.role == role).map(|(_, r)| *r).expect("entry");
        let roles = |d| t.enclosed_by(d).map(|(_, r)| r.role).collect::<Vec<_>>();
        let base = first(RefRole::Base);
        let b = base.entity.expect("the base is resolved");
        let class = base.enclosing.expect("the base is enclosed");
        assert_eq!(roles(class), [RefRole::Base]);

        let init = first(RefRole::MemInit);
        let ctor = init.enclosing.expect("a mem-initializer is enclosed");
        assert_ne!(ctor, class);
        let inits: Vec<_> = t.enclosed_by(ctor).map(|(_, r)| (r.role, r.entity)).collect();
        assert_eq!(inits.len(), 2, "{inits:?}");
        assert_eq!(inits[0], (RefRole::MemInit, Some(b)), "the base initializer names `B`");
        assert_eq!(inits[1].0, RefRole::MemInit);
        assert!(inits[1].1.is_some_and(|m| m != b), "the member initializer names `m`");

        let tmpl = first(RefRole::Template);
        let f = tmpl.enclosing.expect("the return type is enclosed");
        assert!(tmpl.entity.is_some_and(|e| e != b), "`Box` names the class template");
        let ret: Vec<_> = t.enclosed_by(f).map(|(_, r)| (r.role, r.entity)).collect();
        assert_eq!(ret, [(RefRole::Type, Some(b)), (RefRole::Template, tmpl.entity)]);

        // The parameter's type is the parameter's, not `f`'s.
        let param = t
            .iter()
            .filter(|(_, r)| r.role == RefRole::Type && r.entity == Some(b))
            .map(|(_, r)| r.enclosing.expect("enclosed"))
            .find(|&d| d != f && d != class)
            .expect("the parameter's type");
        assert_eq!(roles(param), [RefRole::Type]);
        // Each entry is in the list of its own `enclosing`, once.
        for (id, r) in t.iter() {
            let d = r.enclosing.expect("enclosed");
            assert_eq!(t.enclosed_by(d).filter(|&(i, _)| i == id).count(), 1);
        }
    });
}

const PROVENANCE: &str = "namespace a { struct S {}; void f(S); int v; }\n\
                          using namespace a;\n\
                          void g(int);\n\
                          struct M { int m; template <class T> int t(); };\n\
                          int h(S s, M x) { f(s); g(1); return v + x.m + x.template t<int>(); }\n";

/// Lookup provenance through the API (spec/ast.md §18.7): `how`, the
/// using-directive and the interned argument-dependent sets; off, none of
/// it exists.
#[test]
fn provenance_through_the_api() {
    use kiln_ast::written_refs::RefLookup;
    run_src(PROVENANCE, true, |t| {
        // The expression names of `h`'s body, in order: f, s, g, v, x, x.
        let exprs: Vec<_> = t.iter().filter(|(_, r)| r.role == RefRole::Expr).collect();
        assert_eq!(exprs.len(), 6);
        let (f, fr) = exprs[0];
        let (g, gr) = exprs[2];
        let (v, vr) = exprs[3];
        // `f(s)`: ordinary lookup finds a::f through the directive and
        // argument-dependent lookup again: `Both`; its set is namespace a
        // and class a::S.
        assert_eq!(fr.how, RefLookup::Both);
        let set = t.adl_set(f).expect("an ADL set");
        assert_eq!((set.namespaces().len(), set.classes().len(), set.enums().len()), (1, 1, 0));
        // `g(1)`: ordinary lookup, an empty set (a fundamental argument).
        assert_eq!(gr.how, RefLookup::Unqualified);
        assert!(t.adl_set(g).expect("ADL ran").is_empty());
        assert_ne!(t.adl_set_id(f), t.adl_set_id(g));
        assert_eq!(t.adl_set_count(), 2);
        // `v`, and the parameter type `S`: through the file-scope directive;
        // `S` in `void f(S)` inside `a` is the namespace's own.
        assert_eq!(vr.how, RefLookup::Unqualified);
        let dir = t.using_directive(v).expect("found through the directive");
        let types: Vec<_> = t.iter().filter(|(_, r)| r.role == RefRole::Type).collect();
        assert_eq!(t.using_directive(types[0].0), None);
        assert_eq!(t.using_directive(types[1].0), Some(dir));
        assert_eq!(t.using_directive(f), Some(dir), "`Both` keeps ordinary lookup's directive");
        // `x.m`: a member.
        let (_, mr) = t.iter().find(|(_, r)| r.role == RefRole::Member).expect("x.m");
        assert_eq!(mr.how, RefLookup::Member);
        // `x.template t<int>()`: a member template, role `template`.
        let (_, tr) = t.iter().find(|(_, r)| r.role == RefRole::Template).expect("t<int>");
        assert_eq!(tr.how, RefLookup::Member);
        assert!(t.provenance_mem().0 > 0);
    });
    run_src(PROVENANCE, false, |t| {
        assert_eq!(t.adl_set_count(), 0);
        assert_eq!(t.provenance_mem(), (0, 0));
    });
}

const STAGE3: &str = "namespace n { void f(int); struct S { int a; }; }\n\
                      namespace m = n;\n\
                      using n::f;\n\
                      using namespace m;\n\
                      void n::f(int) {}\n\
                      S s = {.a = 1};\n";

/// Stage-3 entries through the API (spec/ast.md §18.4): every declarative
/// name of `f` has the same entity (its first declaration), a
/// using-declarator's names are enclosed by the using-declaration, a
/// using-directive naming an alias has the alias as its entity, and each
/// gets its `how` (§18.7.2).
#[test]
fn stage3_names_through_the_api() {
    use kiln_ast::written_refs::{REF_MEMBER, RefLookup};
    run_src(STAGE3, true, |t| {
        let only = |role| {
            let v: Vec<_> = t.iter().filter(|(_, r)| r.role == role).map(|(_, r)| *r).collect();
            assert_eq!(v.len(), 1, "one {role:?} entry");
            v[0]
        };
        let alias = only(RefRole::NamespaceAlias);
        let ns = alias.entity.expect("the alias target is namespace n");
        let alias_decl = alias.enclosing.expect("the alias claims its target");
        assert_ne!(alias_decl, ns);
        let udir = only(RefRole::UsingDirective);
        assert_eq!(udir.entity, Some(alias_decl), "a directive naming an alias names the alias");
        assert_eq!(udir.how, RefLookup::Unqualified);

        let using = only(RefRole::UsingDecl);
        let redecl = only(RefRole::Redecl);
        assert!(using.entity.is_some());
        assert_eq!(using.entity, redecl.entity, "both name f's first declaration");
        assert_eq!((using.how, redecl.how), (RefLookup::Qualified, RefLookup::Qualified));
        let decl = using.enclosing.expect("enclosed");
        let roles: Vec<_> = t.enclosed_by(decl).map(|(_, r)| r.role).collect();
        assert_eq!(roles, [RefRole::Nns, RefRole::UsingDecl]);

        let des = only(RefRole::Designator);
        assert!(des.entity.is_some(), "the designator names S::a");
        assert_eq!(des.flags & REF_MEMBER, REF_MEMBER);
        assert_eq!(des.how, RefLookup::Member);
    });
}
