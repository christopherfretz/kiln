//! The written-reference table (spec/ast.md §18): off by default, it
//! records nothing and reports no memory; on, it records each written
//! name once and dumps it, and `enclosed_by` finds a declaration's type,
//! template, base and mem-init names (stage 2); the lookup provenance
//! reads through the API (§18.7); the per-specialization entries find a
//! member's every instantiated use, in any order (§18.9).

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

/// Parses `src` with recording on and hands the AST and the source
/// manager to `check`.
fn with_ast(src: &str, check: impl FnOnce(&kiln_ast::Ast, &SourceManager)) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions { written_refs: true, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    assert_eq!(engine.error_count(), 0, "the source compiles");
    check(&tu.ast, &sm);
}

/// `line:col` of `loc`.
fn line_col(sm: &SourceManager, loc: kiln_source::Loc) -> (u32, u32) {
    let lc = sm.line_col(sm.spelling_loc(loc)).expect("a file location");
    (lc.line, lc.col)
}

const FOUND: &str = "namespace n { void f(long); }\n\
                     void f(int);\n\
                     void f(double);\n\
                     using n::f;\n\
                     void g(int);\n\
                     void unused(int);\n\
                     template <class T> void all(T t) { g(t); }\n\
                     template <class T> void mixed(T t) { f(t); }\n\
                     void use() { unused(1); }\n";

/// Rename by found set (spec/ast.md §18.12.1), in templates no
/// specialization of which is instantiated: `g` is every element of the
/// set of `g(t)` (the name always names it); `unused` is in no set;
/// `f(int)` is one of three elements of the set of `f(t)` (a rename
/// renames a name `f(double)` and `n::f` share), and `n::f` is reached
/// through the using-declaration's shadow.
#[test]
fn rename_by_found_set() {
    with_ast(FOUND, |ast, sm| {
        let t = &ast.written_refs;
        assert_eq!(t.spec_len(), 0, "nothing is instantiated");
        let pos = |d: kiln_ast::DeclId| line_col(sm, ast.decls.get(d).loc());
        // All subjects: `g(t)`.
        let all = entry_at(t, sm, (7, 36));
        let set = t.found_at_definition(all).expect("a set");
        assert!(set.adl() && !set.dependent_base() && !set.unresolved_using());
        let [g] = set.decls else { panic!("one element") };
        assert_eq!((g.decl, pos(g.decl)), (g.found, (5, 6)));
        assert_eq!(t.entries_finding(g.decl).collect::<Vec<_>>(), [all]);
        assert!(set.decls.iter().all(|e| e.decl == g.decl));
        // None: `unused` is named only by its non-dependent call.
        let (_, call) = t.iter().find(|(_, r)| line_col(sm, r.range.lo) == (9, 14)).expect("call");
        let unused = call.entity.expect("resolved");
        assert!(t.found_at_definition(entry_at(t, sm, (9, 14))).is_none(), "not dependent");
        assert_eq!(t.entries_finding(unused).count(), 0);
        // Mixed: `f(t)`.
        let mixed = entry_at(t, sm, (8, 38));
        let set = t.found_at_definition(mixed).expect("a set");
        assert_eq!(set.decls.len(), 3);
        let mut at: Vec<_> = set.decls.iter().map(|e| pos(e.decl)).collect();
        at.sort_unstable();
        assert_eq!(at, [(1, 20), (2, 6), (3, 6)]);
        let fi = set.decls.iter().find(|e| pos(e.decl) == (2, 6)).expect("f(int)");
        assert!(set.contains(fi.decl));
        assert!(set.decls.iter().any(|e| e.decl != fi.decl), "a name others share");
        assert_eq!(t.entries_finding(fi.decl).collect::<Vec<_>>(), [mixed]);
        let nf = set.decls.iter().find(|e| pos(e.decl) == (1, 20)).expect("n::f");
        assert_ne!(nf.found, nf.decl, "through the shadow");
        assert_eq!(pos(nf.found), (4, 10));
        assert_eq!(t.entries_finding(nf.found).collect::<Vec<_>>(), [mixed]);
        assert_eq!(t.entries_finding(nf.decl).collect::<Vec<_>>(), [mixed]);
        // Equal sets are one interned set.
        assert_ne!(t.found_set_id(all), t.found_set_id(mixed));
    });
}

const SPECS: &str = "struct Vec { int size() const { return 1; } };\n\
                     struct List { int size() const { return 2; } };\n\
                     template <class T> int measure(const T &t) { return t.size(); }\n\
                     template <class T> struct Wrap { T t; int n() const { return t.size(); } };\n\
                     int direct(const Vec &v) { return v.size(); }\n\
                     int run() {\n\
                     \x20 int a = measure(Vec{}) + Wrap<Vec>{}.n();\n\
                     \x20 int b = measure(List{}) + Wrap<List>{}.n();\n\
                     \x20 return a + b + direct(Vec{});\n\
                     }\n";

/// Rename by table (spec/ast.md §18.9.1): the written occurrences of
/// `Vec::size` are the main table's entries naming it (the
/// non-dependent call in `direct`) and the pattern entries of the
/// per-specialization entries naming it (the dependent calls, as
/// `measure<Vec>` and `Wrap<Vec>::n` resolve them); `List::size`'s
/// specializations are not among them.
#[test]
fn rename_by_table_finds_every_specialization() {
    with_ast(SPECS, |ast, sm| {
        let t = &ast.written_refs;
        // `Vec::size` from the non-dependent call `v.size()` in `direct`.
        let (_, direct) = t
            .iter()
            .find(|(_, r)| r.role == RefRole::Member && line_col(sm, r.range.lo).0 == 5)
            .expect("v.size()");
        let size = direct.entity.expect("Vec::size");
        let mut at: Vec<(u32, u32)> = t
            .iter()
            .filter(|(_, r)| r.entity == Some(size))
            .map(|(_, r)| line_col(sm, r.range.lo))
            .collect();
        let specs: Vec<_> = t.specs_naming(size).collect();
        assert_eq!(specs.len(), 2, "measure<Vec> and Wrap<Vec>::n");
        for (_, r) in &specs {
            assert_eq!(r.entity, Some(size));
            let p = t.get(r.entry);
            assert!(p.entity.is_none(), "the pattern entry is dependent");
            at.push(line_col(sm, p.range.lo));
        }
        at.sort_unstable();
        assert_eq!(at, [(3, 55), (4, 64), (5, 37)]);
        // The two specializations of `measure` resolve its one entry to
        // two entities.
        let (entry, _) = t
            .iter()
            .find(|(_, r)| r.role == RefRole::Member && line_col(sm, r.range.lo) == (3, 55))
            .expect("t.size() in measure");
        let mut named: Vec<_> = t.specs_of(entry).map(|(_, r)| r.entity).collect();
        named.dedup();
        assert_eq!(named.len(), 2);
        assert!(named.contains(&Some(size)));
        assert_eq!(t.spec_len(), 4);
        assert!(t.spec_mem().0 > 0);
    });
}

/// The per-specialization table does not depend on the order of
/// instantiation (spec/ast.md §18.9.1): with the two uses swapped, the
/// rows, compared as sorted (pattern position, entity position,
/// specialization position) triples, are the same.
#[test]
fn specs_are_order_independent() {
    let swapped = SPECS.replace(
        "  int a = measure(Vec{}) + Wrap<Vec>{}.n();\n  int b = measure(List{}) + Wrap<List>{}.n();\n",
        "  int b = measure(List{}) + Wrap<List>{}.n();\n  int a = measure(Vec{}) + Wrap<Vec>{}.n();\n",
    );
    assert_ne!(swapped, SPECS);
    let rows = |src: &str| {
        let mut v = Vec::new();
        with_ast(src, |ast, sm| {
            let t = &ast.written_refs;
            let pos = |d: kiln_ast::DeclId| line_col(sm, ast.decls.get(d).loc());
            v = t
                .specs()
                .map(|(_, r)| {
                    (line_col(sm, t.get(r.entry).range.lo), r.entity.map(pos), pos(r.spec))
                })
                .collect();
        });
        v.sort_unstable();
        v
    };
    let a = rows(SPECS);
    assert_eq!(a.len(), 4);
    assert_eq!(a, rows(&swapped));
}

/// The pattern entry of the dependent name at `line:col` (its id).
fn entry_at(
    t: &kiln_ast::written_refs::WrittenRefs,
    sm: &SourceManager,
    at: (u32, u32),
) -> kiln_ast::written_refs::WrittenRefId {
    t.iter().find(|(_, r)| line_col(sm, r.range.lo) == at).map(|(id, _)| id).expect("an entry")
}

const SAT: &str = "struct V { int size() const { return 1; } };\n\
                   struct W { int size() const { return 2; } };\n\
                   template <class T> concept Sized = requires(const T &t) { t.size(); };\n\
                   template <class T> requires Sized<T> int f(const T &) { return 1; }\n\
                   template <class T> requires Sized<T> int g(const T &) { return 2; }\n\
                   int run() { return f(V{}) + g(V{}) + g(W{}); }\n";

/// Names in constraints (spec/ast.md §18.9.2, WP-2223): the rows of
/// `t.size()` in the concept are keyed by the concept, one per distinct
/// resolution. `g<V>` satisfies the atom `f<V>` already satisfied (a
/// cache hit that skips the substitution): the rows are the same as
/// when `g<V>` checks it first, and none is there twice.
#[test]
fn constraint_rows_survive_cache_hits() {
    let rows = |src: &str| {
        let mut v = Vec::new();
        with_ast(src, |ast, sm| {
            let t = &ast.written_refs;
            let entry = entry_at(t, sm, (3, 61));
            let pos = |d: kiln_ast::DeclId| line_col(sm, ast.decls.get(d).loc());
            v = t.specs_of(entry).map(|(_, r)| (r.entity.map(pos), pos(r.spec))).collect();
            let n = v.len();
            v.sort_unstable();
            v.dedup();
            assert_eq!(v.len(), n, "no row twice");
        });
        v
    };
    let both = rows(SAT);
    // V::size and W::size, keyed by the concept `Sized` (3:28).
    assert_eq!(both, [(Some((1, 16)), (3, 28)), (Some((2, 16)), (3, 28))]);
    let first_g = SAT.replace("f(V{}) + g(V{})", "g(V{}) + f(V{})");
    assert_ne!(first_g, SAT);
    assert_eq!(rows(&first_g), both);
    let no_f = SAT.replace("f(V{}) + g(V{})", "g(V{})");
    assert_eq!(rows(&no_f), both);
}

/// Using-declarations with a dependent qualifier (spec/ast.md §18.9.4,
/// WP-2222): a rename of `V::size` by the table also edits the
/// `using T::size;` of the class template instantiated with `V`, and the
/// name a `using typename T::t;` introduced, used as a type, has a row
/// naming `V::t` with `found` the shadow.
#[test]
fn rename_by_table_finds_dependent_using_declarations() {
    let src = "struct V { int size() const { return 1; } using t = int; };\n\
               template <class T> struct D : T {\n\
               \x20 using T::size;\n\
               \x20 using typename T::t;\n\
               \x20 t get() const { return size(); }\n\
               };\n\
               int run() { return D<V>{}.get(); }\n";
    with_ast(src, |ast, sm| {
        let t = &ast.written_refs;
        let using = entry_at(t, sm, (3, 12));
        let (_, row) = t.specs_of(using).next().expect("the using-declarator's row");
        let size = row.entity.expect("V::size");
        assert_eq!(line_col(sm, ast.decls.get(size).loc()), (1, 16));
        assert_eq!(row.found, None, "the using-declaration is the declaration");
        let mut at: Vec<(u32, u32)> =
            t.specs_naming(size).map(|(_, r)| line_col(sm, t.get(r.entry).range.lo)).collect();
        at.sort_unstable();
        at.dedup();
        assert_eq!(at, [(3, 12), (5, 26)], "the using-declarator and the call");
        let ty = entry_at(t, sm, (5, 3));
        let (_, row) = t.specs_of(ty).next().expect("the type's row");
        let target = row.entity.expect("V::t");
        assert_eq!(line_col(sm, ast.decls.get(target).loc()), (1, 49));
        let shadow = row.found.expect("the shadow");
        assert_eq!(line_col(sm, ast.decls.get(shadow).loc()), (4, 21));
    });
}

/// The using-directive of a per-specialization row (spec/ast.md §18.9.1,
/// WP-2220): ordinary lookup at the template's definition found `f`
/// through `using namespace a;`, and the row says so.
#[test]
fn spec_rows_name_their_using_directive() {
    let src = "namespace a { void f(int); void f(long); }\n\
               using namespace a;\n\
               template <class T> void run(T t) { f(t); }\n\
               void use() { run(1); }\n";
    with_ast(src, |ast, sm| {
        let t = &ast.written_refs;
        let call = entry_at(t, sm, (3, 36));
        let (_, row) = t.specs_of(call).next().expect("the call's row");
        let udir = row.udir.expect("found through the directive");
        assert_eq!(line_col(sm, ast.decls.get(udir).loc()), (2, 1));
    });
}

/// Thread-safety attribute arguments (spec/ast.md §18.8, WP-2170): a name
/// that lookup does not find is an entry with no entity, as an undeclared
/// name is anywhere else, and is not diagnosed (kiln skips the attribute,
/// WP-606); a member named before its declaration is found once the
/// class is complete.
#[test]
fn thread_safety_arguments_are_entries() {
    let src = "struct M {};\n\
               void f() __attribute__((requires_capability(no_such_mutex)));\n\
               struct S { int x __attribute__((guarded_by(mu))); M mu; };\n";
    with_ast(src, |ast, sm| {
        let t = &ast.written_refs;
        let missing = t.get(entry_at(t, sm, (2, 45)));
        assert!(missing.entity.is_none(), "no entity for a name not found");
        let later = t.get(entry_at(t, sm, (3, 44)));
        let mu = later.entity.expect("the member declared after the attribute");
        assert_eq!(line_col(sm, ast.decls.get(mu).loc()), (3, 53));
    });
}

/// Stage 5 (spec/ast.md §18.10.1): a prefix parsed with the table on,
/// written to a snapshot with its source manager and interner, restored
/// into a fresh `Sema` that records and continued with a main file. A
/// rename of `Vec::size` by the table then finds its uses in both
/// files: the prefix's own call, the prefix's dependent call through
/// the row of the main file's instantiation, and the main file's call;
/// not `List::size`'s.
#[test]
fn rename_by_table_across_a_restored_prefix() {
    use kiln_lex::Std;
    use kiln_parse::{continue_preprocessed, new_sema, parse_prefix};
    use kiln_pp::{PpOptions, Preprocessor};
    use kiln_snap::{Reader, Writer};

    const PREFIX: &str = "namespace lib {\n\
                          struct Vec { int size() const { return 0; } };\n\
                          struct List { int size() const { return 1; } };\n\
                          template <class T> int count(const T &t) { return t.size(); }\n\
                          inline int direct(const Vec &v) { return v.size(); }\n\
                          }\n";
    const MAIN: &str = "int use(const lib::Vec &v, const lib::List &l) {\n\
                        \x20 return lib::count(v) + lib::count(l) + v.size();\n\
                        }\n";
    let target = Target::for_arch(Arch::X86_64);
    let opts = ParseOptions { written_refs: true, ..ParseOptions::default() };
    let pp_opts = |header: bool| {
        let mut o = PpOptions::new(Std::Cxx26);
        o.parser_pragmas = true;
        o.main_is_header = header;
        o
    };
    // The producer: the prefix to its end, then the snapshot.
    let bytes = {
        let sm = SourceManager::new();
        let interner = Arc::new(Interner::new());
        let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
        let file = sm.add_virtual("prefix.h", PREFIX.to_owned()).expect("location space");
        let mut pp = Preprocessor::new(&sm, &engine, &interner, pp_opts(true));
        pp.enter_main_file(file);
        let p = parse_prefix(
            pp,
            Arc::clone(&interner),
            &engine,
            target.clone(),
            &opts,
            ConstexprLimits::default(),
        );
        p.ready.expect("the prefix is at rest");
        assert_eq!(engine.error_count(), 0);
        assert!(p.sema.ast.written_refs.len() >= 3, "the prefix's names are recorded");
        let mut w = Writer::new();
        w.put(&sm);
        w.put(&interner);
        p.sema.snap_put(&mut w).expect("the prefix is at rest");
        w.into_bytes()
    };
    // The consumer: restore, then the main file.
    let mut r = Reader::new(&bytes);
    let sm: SourceManager = r.get().expect("source manager");
    let interner: Arc<Interner> = r.get().expect("interner");
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut s =
        new_sema(Arc::clone(&interner), &engine, target.clone(), &opts, ConstexprLimits::default());
    s.snap_restore(&mut r).expect("restored");
    r.finish().expect("the whole snapshot is read");
    let prefix_entries = s.ast.written_refs.len();
    assert!(prefix_entries >= 3 && s.ast.written_refs.unrecorded().is_empty());
    let file = sm.add_virtual("main.cpp", MAIN.to_owned()).expect("location space");
    let mut pp = Preprocessor::new(&sm, &engine, &interner, pp_opts(false));
    pp.enter_main_file(file);
    let (tu, _pp) = continue_preprocessed(s, pp, &target, &opts);
    assert_eq!(engine.error_count(), 0, "the main file compiles");
    let t = &tu.ast.written_refs;
    assert!(t.len() > prefix_entries, "the main file's entries follow the prefix's");
    let at = |l: kiln_source::Loc| {
        let lc = sm.line_col(sm.spelling_loc(l)).expect("a file location");
        (sm.file_name(lc.file).to_owned(), lc.line, lc.col)
    };
    // `Vec::size` from `v.size()` in `direct` (a prefix entry).
    let (_, direct) = t
        .iter()
        .find(|(_, r)| r.role == RefRole::Member && at(r.range.lo).1 == 5)
        .expect("v.size() in direct");
    let size = direct.entity.expect("Vec::size");
    let mut found: Vec<_> = t
        .iter()
        .filter(|(_, r)| r.entity == Some(size))
        .map(|(_, r)| at(r.range.lo))
        .chain(t.specs_naming(size).map(|(_, r)| at(t.get(r.entry).range.lo)))
        .collect();
    found.sort();
    let want = [
        ("main.cpp".to_owned(), 2, 44),
        ("prefix.h".to_owned(), 4, 53),
        ("prefix.h".to_owned(), 5, 44),
    ];
    assert_eq!(found, want);
}

const LEADS: &str = "namespace a { struct S { ~S(); static int n; }; int f(); }\n\
                     #define G ::\n\
                     int use(a::S *p) {\n\
                     \x20 p->::a::S::~S();\n\
                     \x20 return G a::f() + :: /* c */ a::S::n;\n\
                     }\n";

/// The leading `::` and the destructor's `~` through the API
/// (spec/ast.md §18.11.2, WP-2326): a globally qualified name's first
/// component knows its `::` (a macro's at the macro's token), every
/// component of the name begins its qualified name there, and a
/// destructor name joins its `MemberExpr` at its `~`.
#[test]
fn leading_colons_and_tildes_through_the_api() {
    use kiln_ast::written_refs::{QualLink, RefQualifier};
    with_ast(LEADS, |ast, sm| {
        let t = &ast.written_refs;
        let globals: Vec<_> =
            t.iter().filter(|(_, r)| r.qualifier == RefQualifier::GLOBAL).collect();
        assert_eq!(globals.len(), 3, "a in p->::a, G a and :: a");
        for &(id, _) in &globals {
            assert!(t.global_colon(id).is_some(), "every leading `::` is kept");
        }
        // `p->::a::S::~S()`.
        let first = entry_at(t, sm, (4, 8));
        assert_eq!(line_col(sm, t.global_colon(first).unwrap()), (4, 6));
        let dtor = entry_at(t, sm, (4, 15));
        assert_eq!(t.get(dtor).role, RefRole::Member);
        assert_eq!(line_col(sm, t.tilde(dtor).unwrap()), (4, 14));
        assert_eq!(line_col(sm, t.name_lo(dtor)), (4, 14));
        assert_eq!(line_col(sm, t.qualified_lo(dtor)), (4, 6));
        // `G a::f()`: the `::` is the macro's token, spelled in its body.
        let call = entry_at(t, sm, (5, 15));
        let QualLink::Entry(qual) = t.get(call).qualifier.link() else { panic!("a::f") };
        let colon = t.global_colon(qual).unwrap();
        assert!(sm.is_macro_loc(colon), "the macro's `::`");
        assert_eq!(line_col(sm, colon), (2, 11));
        assert_eq!(t.qualified_lo(call), colon);
        // `:: /* c */ a::S::n`.
        let member = entry_at(t, sm, (5, 38));
        assert_eq!(line_col(sm, t.qualified_lo(member)), (5, 21));
        assert_eq!(t.name_lo(member), t.get(member).range.lo);
        // The `MemberExpr` of the destructor is at its `~`.
        let mut joined = false;
        for i in 1..=ast.exprs.len() {
            let expr = kiln_ast::ExprId::new(i as u32);
            if ast.exprs.get(expr).kind() == kiln_ast::expr::ExprKind::MemberExpr
                && ast.exprs.get(expr).loc() == t.name_lo(dtor)
            {
                assert_eq!(ast.exprs.decl(expr), t.get(dtor).entity.unwrap());
                joined = true;
            }
        }
        assert!(joined, "the destructor's MemberExpr joins its entry");
    });
    run_src(LEADS, false, |t| {
        assert!(t.is_empty() && t.leads().is_empty());
    });
}

const OFFSETOF: &str = "struct In { int c; int d[3]; };\n\
                        struct M { int x; In b[2]; };\n\
                        struct S { int q; M a; union { int u; float f; }; };\n\
                        unsigned long o1 = __builtin_offsetof(S, a.b[1].c);\n\
                        unsigned long o2 = __builtin_offsetof(S, u);\n\
                        template <int N> unsigned long o3 = __builtin_offsetof(S, a.b[N].nope.c);\n\
                        template <class T> unsigned long o4 = __builtin_offsetof(T, a.b);\n";

/// The member names of an offsetof designator ([support.types.layout]/1;
/// spec/ast.md §18.4, WP-2471): each component is an entry naming the
/// member the expression's resolved designator holds at that name
/// (`ExprArena::offsetof_designator`, the last `Field` node ending
/// there: a union's member, not the anonymous union); with a
/// value-dependent index the names into a non-dependent type bind at
/// the definition, the one lookup does not find and every name after it
/// are `UNRESOLVED` (not `DEPENDENT`), and the names into a dependent
/// type are `DEPENDENT`.
#[test]
fn offsetof_designators_are_entries_support_types_layout_1() {
    use kiln_ast::ExprId;
    use kiln_ast::expr::{ExprKind, OffsetofNode};
    use kiln_ast::written_refs::{REF_DEPENDENT, REF_MEMBER, REF_UNRESOLVED, WrittenRef};
    with_ast(OFFSETOF, |ast, sm| {
        let des: Vec<WrittenRef> = ast
            .written_refs
            .iter()
            .map(|(_, r)| *r)
            .filter(|r| r.role == RefRole::Designator)
            .collect();
        assert!(des.iter().all(|r| r.flags & REF_MEMBER != 0));
        let line = |r: &WrittenRef| line_col(sm, r.range.lo).0;
        let on = |l: u32| -> Vec<(String, bool, u8)> {
            des.iter()
                .filter(|r| line(r) == l)
                .map(|r| {
                    let text = sm.span_text(r.range).unwrap_or("?").to_owned();
                    (text, r.entity.is_some(), r.flags & (REF_DEPENDENT | REF_UNRESOLVED))
                })
                .collect()
        };
        let named = |v: &[(&str, bool, u8)]| -> Vec<(String, bool, u8)> {
            v.iter().map(|&(n, e, f)| (n.to_owned(), e, f)).collect()
        };
        assert_eq!(on(4), named(&[("a", true, 0), ("b", true, 0), ("c", true, 0)]));
        assert_eq!(on(5), named(&[("u", true, 0)]));
        assert_eq!(
            on(6),
            named(&[
                ("a", true, 0),
                ("b", true, 0),
                ("nope", false, REF_UNRESOLVED),
                ("c", false, REF_UNRESOLVED),
            ])
        );
        assert_eq!(on(7), named(&[("a", false, REF_DEPENDENT), ("b", false, REF_DEPENDENT)]));
        // Each entry of a non-dependent designator names the member its
        // resolved designator holds at that name.
        let mut checked = 0;
        for e in (1..=ast.exprs.len()).map(|i| ExprId::new(u32::try_from(i).expect("index"))) {
            if ast.exprs.get(e).kind() != ExprKind::OffsetOfExpr {
                continue;
            }
            let Some((_, nodes)) = ast.exprs.offsetof_designator(e) else { continue };
            let field = |r: &WrittenRef| {
                nodes.iter().rev().find_map(|n| match *n {
                    OffsetofNode::Field { field, end, .. } if end == r.range.lo => Some(field),
                    _ => None,
                })
            };
            for r in &des {
                if let Some(f) = field(r) {
                    assert_eq!(r.entity, Some(f), "{:?}", sm.span_text(r.range));
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 4, "a, b, c and u");
    });
}

const PACK_AND_MEMBER: &str = "template <class... Ts> int first(Ts... ts) { return ts...[0]; }\n\
                               struct V { template <int N> static constexpr int twice = 2 * N; };\n\
                               int use(V d) { return first(1, 2) + d.twice<7>; }\n";

/// The pack's name in `ts...[0]` names the pack, not a dependent name
/// (WP-2721, spec/ast.md §18.5); a member variable template's
/// specialization `d.twice<7>` has the template as its found set and
/// keeps its template arguments outside a template (WP-2724).
#[test]
fn pack_index_and_member_variable_template_names() {
    use kiln_ast::written_refs::REF_DEPENDENT;
    with_ast(PACK_AND_MEMBER, |ast, sm| {
        let t = &ast.written_refs;
        let at = |lc| t.iter().find(|(_, r)| line_col(sm, r.range.lo) == lc).map(|(_, r)| *r);
        let ts = at((1, 53)).expect("an entry for `ts` in `ts...[0]`");
        assert_eq!(ts.role, RefRole::Expr);
        assert_eq!(ts.flags & REF_DEPENDENT, 0, "only the indexing is dependent");
        let pack = ts.entity.expect("the pack");
        let name = ast.decls.get(pack).name().as_ident().map(|s| ast.symbols().as_str(s));
        assert_eq!(name, Some("ts"));
        let twice = at((3, 39)).expect("an entry for `twice`");
        assert_eq!(twice.role, RefRole::Member);
        assert!(twice.found.is_some(), "the member template is the found set");
        let targs = kiln_ast::dump::written_exprs(ast)
            .into_iter()
            .filter(|&e| ast.exprs.get(e).kind() == kiln_ast::expr::ExprKind::MemberExpr)
            .filter_map(|e| ast.exprs.decl_ref_extra(e).and_then(|x| x.template_args))
            .count();
        assert_eq!(targs, 1, "`d.twice<7>` keeps its template arguments");
    });
}

/// `line:col` of the last `needle` on 1-based `line` of `src`.
fn pos_in(src: &str, line: u32, needle: &str) -> (u32, u32) {
    let text = src.lines().nth(line as usize - 1).expect("the line");
    let col = text.rfind(needle).expect("the needle") + 1;
    (line, u32::try_from(col).expect("a short line"))
}

const FRIEND_SPEC: &str = "namespace n {\n\
                           template <class T> void f(T) {}\n\
                           template <class T> void f(T, int) {}\n\
                           template <class T> class A { friend void f<>(T, int); };\n\
                           template <class T> class N { friend void f<int>(int); };\n\
                           }\n\
                           void run() { n::A<int> a; n::A<char> c; }\n";

/// A friend function template specialization in a template (spec/ast.md
/// §18.4, WP-3008): dependent, the entry's found set is the candidate
/// templates, its written `<>` is the lead's argument list, and each
/// instantiation's row names the specialization of the template the
/// declarator matches (the second overload, not lookup's first). A non-dependent one
/// in a template never instantiated names its matched template.
#[test]
fn friend_specializations_name_their_template() {
    use kiln_ast::written_refs::REF_DEPENDENT;
    with_ast(FRIEND_SPEC, |ast, sm| {
        let t = &ast.written_refs;
        let pos = |d: kiln_ast::DeclId| line_col(sm, ast.decls.get(d).loc());
        let first = pos_in(FRIEND_SPEC, 2, "f(");
        let second = pos_in(FRIEND_SPEC, 3, "f(");
        let entry = entry_at(t, sm, pos_in(FRIEND_SPEC, 4, "f<"));
        let pat = t.get(entry);
        assert!(pat.entity.is_none() && pat.flags & REF_DEPENDENT != 0, "dependent");
        let set = t.found_at_definition(entry).expect("a found set");
        let mut at: Vec<_> = set.decls.iter().map(|e| pos(e.decl)).collect();
        at.sort_unstable();
        assert_eq!(at, [first, second], "the candidate templates");
        let (langle, rangle) = t.template_args(entry).expect("the written arguments");
        assert_eq!(line_col(sm, langle), pos_in(FRIEND_SPEC, 4, "<>"));
        assert_eq!(line_col(sm, rangle), pos_in(FRIEND_SPEC, 4, ">("));
        let rows: Vec<_> = t.specs_of(entry).map(|(_, s)| *s).collect();
        assert_eq!(rows.len(), 2, "A<int> and A<char>");
        for row in &rows {
            assert_eq!(row.found.map(pos), Some(second));
            assert_eq!(row.entity.map(pos), Some(second), "a specialization of it");
            assert_ne!(row.entity, row.found);
        }
        assert_ne!(rows[0].entity, rows[1].entity);
        let plain = t.get(entry_at(t, sm, pos_in(FRIEND_SPEC, 5, "f<")));
        assert_eq!(plain.flags & REF_DEPENDENT, 0);
        assert_eq!(plain.entity.map(pos), Some(first));
    });
}

const FRIEND_MEMBER: &str = "namespace n {\n\
                             template <class T> struct S { void f(); };\n\
                             template <class T> struct S<T *> { void f(); };\n\
                             }\n\
                             class X {\n\
                             \x20 template <class T> friend void n::S<T>::f();\n\
                             \x20 template <class T> friend void n::S<T *>::f();\n\
                             };\n";

/// A friend member of a dependent specialization ([temp.friend]/5,
/// spec/ast.md §18.4, WP-3008): `n::S<T>::f` names the primary's
/// member, `n::S<T *>::f` the partial specialization's.
#[test]
fn friend_members_name_the_pattern_member() {
    with_ast(FRIEND_MEMBER, |ast, sm| {
        let t = &ast.written_refs;
        let pos = |d: kiln_ast::DeclId| line_col(sm, ast.decls.get(d).loc());
        for (line, member) in [(6, 2), (7, 3)] {
            let r = t.get(entry_at(t, sm, pos_in(FRIEND_MEMBER, line, "f(")));
            assert_eq!(r.role, RefRole::Elaborated);
            assert_eq!(r.entity.map(pos), Some(pos_in(FRIEND_MEMBER, member, "f(")));
        }
    });
}

const VAR_TEMPLATE_ID: &str = "namespace n { template <class T> constexpr bool v = true; }\n\
                               template <class T> bool k() { return n::v<T>; }\n\
                               bool z = k<int>();\n";

/// A variable template-id (spec/ast.md §18.5, WP-3009): `n::v<T>` names
/// the variable template, as a class template-id names its template.
#[test]
fn variable_template_ids_name_the_template() {
    use kiln_ast::written_refs::REF_DEPENDENT;
    with_ast(VAR_TEMPLATE_ID, |ast, sm| {
        let t = &ast.written_refs;
        let r = t.get(entry_at(t, sm, pos_in(VAR_TEMPLATE_ID, 2, "v<")));
        assert_eq!(r.flags & REF_DEPENDENT, 0);
        let v = r.entity.expect("the variable template");
        assert_eq!(ast.decls.get(v).kind(), kiln_ast::decl::DeclKind::VarTemplate);
    });
}

const UDL: &str = "using Z = decltype(sizeof 0);\n\
                   int operator\"\"_n(unsigned long long v) { return int(v); }\n\
                   Z operator\"\"_s(const char *, Z n) { return n; }\n\
                   Z operator\"\"_s(const char8_t *, Z n) { return n; }\n\
                   Z operator\"\"_s(const char16_t *, Z n) { return n; }\n\
                   Z operator\"\"_s(const char32_t *, Z n) { return n; }\n\
                   Z operator\"\"_s(const wchar_t *, Z n) { return n; }\n\
                   int operator\"\"_\u{e9}(unsigned long long v) { return int(v); }\n\
                   #define BODY 3_n\n\
                   #define ARG(x) x\n\
                   #define PASTE(n) n##_n\n\
                   int a = 1_n + ARG(2_n) + BODY + PASTE(4);\n\
                   int b = 5_\\\n\
                   n;\n\
                   Z c = R\"x(r)x\"_s + u8\"p\"_s + L\"q\"_s + u\"r\"_s + U\"s\"_s;\n\
                   int d = 6_\\u00e9;\n";

/// The range of a ud-suffix entry (spec/ast.md §18.13.4): the suffix's
/// own bytes, so its editable file range is the suffix in a file and in
/// a macro argument, none in a macro body or from `##` (no file bytes
/// spell it alone); a line splice in the suffix is inside the range, a
/// universal-character-name is spelled verbatim; raw and prefixed
/// strings end in their suffix the same way. Never the token's last
/// `len` cleaned bytes.
#[test]
fn ud_suffix_ranges_are_the_suffix_bytes() {
    with_ast(UDL, |ast, sm| {
        let t = &ast.written_refs;
        let suffixes: Vec<_> =
            t.iter().filter(|(_, r)| r.role == RefRole::UdSuffix).map(|(_, r)| *r).collect();
        assert_eq!(suffixes.len(), 11, "every suffixed literal");
        let text = |r: &kiln_ast::written_refs::WrittenRef| {
            sm.editable_file_range(r.range).map(|(_, lo, hi)| &UDL[lo as usize..hi as usize])
        };
        let got: Vec<Option<&str>> = suffixes.iter().map(text).collect();
        assert_eq!(
            got,
            [
                Some("_n"),
                Some("_n"),
                None,
                None,
                Some("_\\\nn"),
                Some("_s"),
                Some("_s"),
                Some("_s"),
                Some("_s"),
                Some("_s"),
                Some("_\\u00e9"),
            ]
        );
        for r in &suffixes {
            assert!(r.entity.is_some(), "every suffix names its literal operator");
            let f = r.entity.unwrap();
            assert_eq!(
                ast.decls.get(f).name().kind(),
                kiln_ast::names::DeclNameKind::LiteralOperator,
                "a literal operator"
            );
        }
        // The literal operator call joins its entry: its callee is at the
        // suffix.
        let at_suffix = (1..=ast.exprs.len())
            .map(|i| kiln_ast::ExprId::new(i as u32))
            .filter(|&e| ast.is_udl_call(e))
            .all(|e| {
                let mut c = ast.exprs.callee(e);
                while ast.exprs.get(c).kind() == kiln_ast::expr::ExprKind::ImplicitCastExpr {
                    c = ast.exprs.operand(c);
                }
                let lo = ast.exprs.get(c).loc();
                suffixes.iter().any(|r| r.range.lo == lo)
            });
        assert!(at_suffix, "every literal operator callee is at a suffix entry");
    });
}

const OPS: &str = "struct V { int &operator[](int); int operator()(int) const; };\n\
                   struct O { int operator<=>(const O &) const; bool operator==(const O &) const; };\n\
                   int use(V v, O a, O b) {\n\
                   \x20 bool r = a < b, s = a != b;\n\
                   \x20 return v[1] + v(2) + r + s;\n\
                   }\n";

/// Operator entries through the API (spec/ast.md §18.13): a `[]` or
/// `()` entry spans its opening token and keeps its closing one
/// (`WrittenRefs::close`), how is `member` for a member operator, and a
/// rewritten comparison's entry is flagged.
#[test]
fn operator_entries_through_the_api() {
    use kiln_ast::written_refs::{REF_REVERSED, REF_REWRITTEN, RefLookup};
    with_ast(OPS, |ast, sm| {
        let t = &ast.written_refs;
        let sub = entry_at(t, sm, (5, 11));
        assert_eq!(t.get(sub).role, RefRole::Operator);
        assert_eq!(t.get(sub).how, RefLookup::Member);
        assert_eq!(line_col(sm, t.close(sub).expect("a `]`")), (5, 13));
        let call = entry_at(t, sm, (5, 18));
        assert_eq!(line_col(sm, t.close(call).expect("a `)`")), (5, 20));
        let less = entry_at(t, sm, (4, 14));
        assert_eq!(t.get(less).flags & (REF_REWRITTEN | REF_REVERSED), REF_REWRITTEN);
        assert!(t.close(less).is_none());
        let ne = entry_at(t, sm, (4, 25));
        assert_eq!(t.get(ne).flags & REF_REWRITTEN, REF_REWRITTEN);
        let name = |id| {
            let mut s = String::new();
            kiln_ast::types::TypeNames::write_decl_name(
                ast,
                t.get(id).entity.unwrap(),
                true,
                &mut s,
            );
            s
        };
        assert_eq!(name(less), "O::operator<=>");
        assert_eq!(name(ne), "O::operator==");
    });
}

const DEP_OPS: &str = "struct S { int v; };\n\
                       S operator+(S, S);\n\
                       struct P { S *operator->(); };\n\
                       struct Q { P operator->(); };\n\
                       template <class T> T add(T a, T b) { return a + b; }\n\
                       template <class T> int get(T t) { return t->v; }\n\
                       template <class... T> S sum(T... t) { return (S{} + ... + t); }\n\
                       int run() { S s{}; add(s, s); add(1, 2); sum(s, s); return get(Q{}); }\n";

/// spec/ast.md §18.13.6 through the API: a dependent operator is a
/// DEPENDENT entry whose found set holds the non-member candidates of the
/// definition with the ADL flag ([over.match.oper]/3.2,
/// [temp.dep.candidate]/1); its rows name the operator function each
/// specialization selected, from that set, or are built-in rows naming
/// nothing ([over.built]); a `->` chain's rows are in application order
/// ([over.ref]/1); a binary fold has an entry per token ([expr.prim.fold]/1).
#[test]
fn dependent_operator_entries_through_the_api() {
    use kiln_ast::written_refs::{REF_DEPENDENT, RefLookup, SPEC_BUILTIN};
    with_ast(DEP_OPS, |ast, sm| {
        let t = &ast.written_refs;
        let pos = |d: kiln_ast::DeclId| line_col(sm, ast.decls.get(d).loc());
        let plus = entry_at(t, sm, (5, 47));
        assert_eq!(t.get(plus).role, RefRole::Operator);
        assert_ne!(t.get(plus).flags & REF_DEPENDENT, 0);
        assert!(t.get(plus).entity.is_none());
        let set = t.found_at_definition(plus).expect("a set");
        assert!(set.adl());
        let [op] = set.decls else { panic!("one candidate") };
        assert_eq!(pos(op.decl), (2, 3));
        let rows: Vec<_> = t.specs_of(plus).map(|(_, r)| *r).collect();
        assert_eq!(rows.len(), 2, "add<S> and add<int>");
        let user = rows.iter().find(|r| r.entity.is_some()).expect("add<S>");
        assert_eq!(ast.spec_origin(user), Some(op.decl));
        assert_eq!(user.flags & SPEC_BUILTIN, 0);
        let builtin = rows.iter().find(|r| r.entity.is_none()).expect("add<int>");
        assert_ne!(builtin.flags & SPEC_BUILTIN, 0);
        assert_eq!(builtin.how, RefLookup::Builtin);
        assert_ne!(user.spec, builtin.spec);
        // `t->v` with `T = Q`: `Q::operator->`, then `P::operator->`.
        let arrow = entry_at(t, sm, (6, 43));
        assert_eq!(t.get(arrow).role, RefRole::Operator);
        assert!(t.found_at_definition(arrow).is_none(), "a member only");
        let chain: Vec<_> = t
            .specs_of(arrow)
            .map(|(_, r)| {
                assert_eq!(r.how, RefLookup::Member);
                pos(r.entity.expect("an operator->"))
            })
            .collect();
        assert_eq!(chain, [(4, 14), (3, 15)]);
        // `(S{} + ... + t)`: two tokens, two entries, each with its row.
        for at in [(7, 51), (7, 57)] {
            let e = entry_at(t, sm, at);
            assert_eq!(t.found_at_definition(e).map(|s| s.decls.len()), Some(1));
            let named: Vec<_> = t.specs_of(e).filter_map(|(_, r)| r.entity).collect();
            assert_eq!(named.len(), 1, "one distinct resolution at {at:?}");
        }
    });
}

/// The kinds of the declarations that hold the entries naming `S` (the
/// first record), in recording order.
fn s_holders(ast: &kiln_ast::Ast) -> Vec<kiln_ast::decl::DeclKind> {
    let t = &ast.written_refs;
    let s = t.iter().find(|(_, r)| r.role == RefRole::Type).and_then(|(_, r)| r.entity);
    t.iter()
        .filter(|(_, r)| r.role == RefRole::Type && r.entity == s)
        .map(|(_, r)| ast.decls.get(r.enclosing.expect("enclosed")).kind())
        .collect()
}

/// spec/ast.md §18.3 (WP-3143): an alias, an enumerator, a template's head
/// and a `static_assert` hold the names written in them, as a typedef
/// does; `enclosed_by` finds them there.
#[test]
fn declarations_without_a_declarator_hold_their_names() {
    use kiln_ast::decl::DeclKind as K;
    const SRC: &str = "struct S {};\n\
                       using A = S;\n\
                       typedef S T;\n\
                       enum E { e = sizeof(S) };\n\
                       template <class U = S> void t() {}\n\
                       struct C { using I = S; };\n\
                       static_assert(sizeof(S) == 1);\n";
    with_ast(SRC, |ast, _| {
        assert_eq!(
            s_holders(ast),
            [K::TypeAlias, K::Typedef, K::EnumConstant, K::Function, K::TypeAlias, K::StaticAssert]
        );
        let t = &ast.written_refs;
        let sa = t.iter().last().and_then(|(_, r)| r.enclosing).expect("the static_assert");
        assert_eq!(t.enclosed_by(sa).count(), 1);
        // The template's head and its body share one holder: the function.
        let f = t.iter().find(|(_, r)| ast.decls.get(r.enclosing.unwrap()).kind() == K::Function);
        assert!(f.is_some());
    });
}

/// spec/ast.md §18.3.1 (WP-3144): a shared decl-specifier-seq is held by
/// its declarator group: `REF_GROUP` and `holders` give every declaration,
/// `enclosing` the first; a group of one and a structured binding have
/// one holder.
#[test]
fn a_declarator_group_holds_its_specifier() {
    use kiln_ast::written_refs::REF_GROUP;
    const SRC: &str = "struct S { int w; };\n\
                       S a, b;\n\
                       struct C { S m, n, o; };\n\
                       typedef S T1, T2;\n\
                       S f(), v;\n\
                       S one;\n\
                       S arr[2];\n\
                       auto [x] = S{1};\n";
    with_ast(SRC, |ast, _| {
        let t = &ast.written_refs;
        let s = t.iter().find(|(_, r)| r.role == RefRole::Type).and_then(|(_, r)| r.entity);
        let sizes: Vec<usize> = t
            .iter()
            .filter(|(_, r)| r.role == RefRole::Type && r.entity == s)
            .map(|(id, r)| {
                let h = t.holders(id);
                assert_eq!(h.first().copied(), r.enclosing, "the first holder is `enclosing`");
                assert_eq!(r.flags & REF_GROUP != 0, h.len() > 1);
                if h.len() > 1 {
                    assert_eq!(t.group(h[0]), Some(h));
                }
                h.len()
            })
            .collect();
        // `S a, b`, `S m, n, o`, `typedef S T1, T2`, `S f(), v`, `S one`,
        // `S arr[2]`, then the binding's `S{1}`.
        assert_eq!(sizes[..6], [2, 3, 2, 2, 1, 1]);
        assert!(sizes[6..].iter().all(|&n| n == 1), "{sizes:?}");
        assert_eq!(t.groups().count(), 4);
    });
}

const ALLOC_OPS: &str = "using size_t = decltype(sizeof 0);\n\
                         struct C { static void *operator new(size_t); static void operator delete(void *); };\n\
                         template <class T> T *make() { return ::new T; }\n\
                         void use() { C *c = new C; delete c; delete[] new int[2]; delete make<C>(); }\n";

/// spec/ast.md §18.13.6 (stage 6c) through the API: the `new` and
/// `delete` keywords are `operator` entries naming the allocation and
/// deallocation functions selected ([expr.new]/12, [expr.delete]/9), the
/// class's own or the global one, `how=qualified`; `delete[]` one range
/// over its three tokens; `::new` qualifier GLOBAL with the `::` as its
/// lead; a dependent `::new T` a DEPENDENT entry with no found set whose
/// row names what the specialization selected (the global function,
/// since `::new` skips the class).
#[test]
fn alloc_entries_through_the_api() {
    use kiln_ast::written_refs::{QualLink, REF_DEPENDENT, RefLookup};
    with_ast(ALLOC_OPS, |ast, sm| {
        let t = &ast.written_refs;
        let pos = |d: kiln_ast::DeclId| line_col(sm, ast.decls.get(d).loc());
        let class_fn = |d: kiln_ast::DeclId| {
            ast.decls.get(d).ctx().and_then(|c| ast.decls.record(c)).is_some()
        };
        let new_c = entry_at(t, sm, (4, 21));
        let r = t.get(new_c);
        assert_eq!(r.role, RefRole::Operator);
        assert_eq!(r.how, RefLookup::Qualified);
        assert_eq!(pos(r.entity.expect("C::operator new")), (2, 25));
        let del_c = entry_at(t, sm, (4, 28));
        assert_eq!(pos(t.get(del_c).entity.expect("C::operator delete")), (2, 59));
        // `delete[]` over its three tokens, the global `operator delete[]`.
        let del_arr = entry_at(t, sm, (4, 38));
        assert_eq!(line_col(sm, t.get(del_arr).range.hi), (4, 46));
        assert!(!class_fn(t.get(del_arr).entity.expect("the global operator delete[]")));
        // `::new T`: GLOBAL, `::` the lead, DEPENDENT, no set; its row for
        // `make<C>` names the global `operator new`, not `C`'s.
        let gnew = entry_at(t, sm, (3, 41));
        let g = t.get(gnew);
        assert_eq!(g.qualifier.link(), QualLink::Global);
        assert_eq!(t.global_colon(gnew).map(|c| line_col(sm, c)), Some((3, 39)));
        assert_ne!(g.flags & REF_DEPENDENT, 0);
        assert!(t.found_at_definition(gnew).is_none());
        let rows: Vec<_> = t.specs_of(gnew).map(|(_, r)| *r).collect();
        let [row] = rows.as_slice() else { panic!("one row, make<C>: {rows:?}") };
        assert!(!class_fn(row.entity.expect("the global operator new")));
        assert_eq!(row.how, RefLookup::Qualified);
    });
}

const TEMPLATE_IDS: &str = "namespace n {\n\
                            template <class T> constexpr int v = 0;\n\
                            template <int N> struct I {};\n\
                            template <class T> int one(T) { return 0; }\n\
                            }\n\
                            template <class T> struct S {\n\
                            \x20 template <class U> static constexpr int mw = 1;\n\
                            \x20 int a() { return this->mw<T> + this->template mw<T> + mw<int>; }\n\
                            };\n\
                            template <class T> int g(T t) {\n\
                            \x20 n::I<n::v<T>> i;\n\
                            \x20 return n::v<T> + n::v<int> + n::v<n::I<n::v<T>>> + n::one<T>(t);\n\
                            }\n\
                            int h() { return n::v<long> + g(1) + S<int>().a(); }\n";

/// Every column of `needle` in line `line` of `src`, 1-based, plus
/// `skip` (the offset of the name within the needle).
fn cols_in(src: &str, line: u32, needle: &str, skip: usize) -> Vec<(u32, u32)> {
    let text = src.lines().nth(line as usize - 1).expect("the line");
    text.match_indices(needle)
        .map(|(i, _)| (line, u32::try_from(i + skip + 1).expect("a short line")))
        .collect()
}

/// A rename of a template claims every written token of its name
/// (spec/ast.md §18.5, [temp.names]/2, [temp.res.general]/1; WP-3405):
/// `n::v` written with dependent arguments (`n::v<T>`, nested in another
/// template-id's arguments and in a declaration) names the variable
/// template, as the non-dependent `n::v<int>` names its specialization
/// found as the template; a member variable template of the current
/// instantiation after `this->`, with or without `template`, names it
/// too. A function template-id with dependent arguments is an overload
/// set: the rename claims it through its found set (§18.12.1).
#[test]
fn rename_claims_dependent_template_ids() {
    use kiln_ast::written_refs::REF_DEPENDENT;
    with_ast(TEMPLATE_IDS, |ast, sm| {
        let t = &ast.written_refs;
        let pos = |d: kiln_ast::DeclId| line_col(sm, ast.decls.get(d).loc());
        let claims = |d: kiln_ast::DeclId| {
            let mut at: Vec<(u32, u32)> = t
                .iter()
                .filter(|(_, r)| r.entity == Some(d) || r.found == Some(d))
                .map(|(_, r)| line_col(sm, r.range.lo))
                .chain(t.entries_finding(d).map(|id| line_col(sm, t.get(id).range.lo)))
                .collect();
            at.sort_unstable();
            at
        };
        // `n::v`, from its non-dependent use in `h`.
        let r = t.get(entry_at(t, sm, pos_in(TEMPLATE_IDS, 14, "v<")));
        let v = r.found.expect("found as the template");
        assert_ne!(r.entity, Some(v), "the specialization");
        assert_eq!(pos(v), pos_in(TEMPLATE_IDS, 2, "v ="));
        let mut want = cols_in(TEMPLATE_IDS, 11, "n::v<", 3);
        want.extend(cols_in(TEMPLATE_IDS, 12, "n::v<", 3));
        want.extend(cols_in(TEMPLATE_IDS, 14, "n::v<", 3));
        want.sort_unstable();
        assert_eq!(want.len(), 6);
        assert_eq!(claims(v), want);
        for &at in &want[..5] {
            let r = t.get(entry_at(t, sm, at));
            assert_eq!(r.flags & REF_DEPENDENT, 0, "{at:?} is resolved at the definition");
        }
        // `S::mw`, from `mw<int>`: in the class pattern a member of the
        // current instantiation names the member template itself.
        let r = t.get(entry_at(t, sm, pos_in(TEMPLATE_IDS, 8, "mw<int")));
        let mw = r.entity.expect("the member template");
        assert_eq!(ast.decls.get(mw).kind(), kiln_ast::decl::DeclKind::VarTemplate);
        let want = cols_in(TEMPLATE_IDS, 8, "mw<", 0);
        assert_eq!(want.len(), 3);
        assert_eq!(claims(mw), want);
        // `n::one<T>(t)`: dependent, claimed through its found set.
        let call = entry_at(t, sm, pos_in(TEMPLATE_IDS, 12, "one<"));
        assert_ne!(t.get(call).flags & REF_DEPENDENT, 0);
        let set = t.found_at_definition(call).expect("a found set");
        let [one] = set.decls else { panic!("one template") };
        assert!(set.is_closed());
        assert_eq!(pos(one.decl), pos_in(TEMPLATE_IDS, 4, "one("));
        assert_eq!(claims(one.decl), [pos_in(TEMPLATE_IDS, 12, "one<")]);
    });
}

/// A function template's default template argument (spec/ast.md §18.9.2,
/// WP-3520): the deduction that uses it forms `call<n::S, 4>`, and the
/// default's `s` has one row, for that specialization, naming
/// `n::S::s`. A dependent call with one candidate (§18.9.4) gets a row
/// only where the candidate is viable: `Sw<n::S>` records `n::swap` with
/// `n::S`'s associated set, the unsatisfied `Sw<n::U>` records nothing.
#[test]
fn default_template_arguments_and_one_candidate_calls() {
    let src = "namespace n { struct S { static int s() { return 1; } }; struct U {}; void swap(S &, S &); }\n\
               template <class T, int = sizeof(T::s())> int call() { return 0; }\n\
               template <class T> concept Sw = requires(T &a) { swap(a, a); };\n\
               int u() { return call<n::S>() + call<n::S, 4>(); }\n\
               static_assert(Sw<n::S> && !Sw<n::U>);\n";
    with_ast(src, |ast, sm| {
        let t = &ast.written_refs;
        let spec = t.get(entry_at(t, sm, (4, 18))).entity.expect("call<n::S, 4>");
        let s = entry_at(t, sm, (2, 36));
        let rows: Vec<_> = t.specs_of(s).map(|(_, r)| *r).collect();
        assert_eq!(rows.len(), 1, "one row for the default: {rows:?}");
        assert_eq!(rows[0].spec, spec, "keyed by the formed specialization");
        let e = rows[0].entity.expect("n::S::s");
        assert_eq!(line_col(sm, ast.decls.get(e).loc()), (1, 37));
        let swap = entry_at(t, sm, (3, 50));
        let rows: Vec<_> = t.specs_of(swap).map(|(_, r)| *r).collect();
        assert_eq!(rows.len(), 1, "no row for the unsatisfied check: {rows:?}");
        let f = rows[0].entity.expect("n::swap");
        assert_eq!(line_col(sm, ast.decls.get(f).loc()), (1, 76));
        let set = t.adl_set_of(rows[0].adl.expect("the associated set"));
        let classes: Vec<_> =
            set.classes().iter().map(|&c| line_col(sm, ast.decls.get(c).loc())).collect();
        assert_eq!(classes, [(1, 22)], "n::S's set");
    });
}

/// Parses `src` with the table on and the substitution keys `keys`, and
/// returns the dump, the keyed table's bytes and its memory row.
fn run_keys(src: &str, keys: bool) -> (String, (u64, u64), Option<kiln_ast::mem::MemRow>) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts =
        ParseOptions { written_refs: true, written_ref_keys: keys, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let t = &tu.ast.written_refs;
    assert_eq!(t.subst_keys_on(), keys);
    let row = tu.ast.memory().rows.into_iter().find(|r| r.name == "written_refs.subst");
    (t.dump(&tu.ast, &sm, Some(file)), t.subst_mem(), row)
}

/// Substitution keys, stage 0 (spec/ast.md §18.14.7): with the table on
/// and keys off, a parse that instantiates templates, satisfies
/// constraints and expands an alias template allocates nothing for keys
/// and reports no memory row; with keys on (no compiler path opens a key
/// scope yet) the dump is the same and the `written_refs.subst` row
/// counts 0 rows.
#[test]
fn keys_off_allocate_nothing_and_keys_on_change_no_dump() {
    let sat = SAT.replace("int run()", "int run_sat()");
    let src = format!(
        "{SPECS}{sat}template <class T> using Size = decltype(T{{}}.size());\n\
                       Size<V> s = 0;\n"
    );
    let (off, mem, row) = run_keys(&src, false);
    assert_eq!(mem, (0, 0));
    assert!(row.is_none(), "no row with keys off");
    assert!(off.contains("spec="), "{off}");
    let (on, _, row) = run_keys(&src, true);
    assert_eq!(on, off);
    let row = row.expect("a row with keys on");
    assert_eq!(row.count, 0);
}

/// Parses `src` with the table and the substitution keys on (the source
/// compiles) and hands the AST and the source manager to `check`.
fn with_keys(src: &str, check: impl FnOnce(&kiln_ast::Ast, &SourceManager)) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts =
        ParseOptions { written_refs: true, written_ref_keys: true, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    assert_eq!(engine.error_count(), 0, "the source compiles");
    check(&tu.ast, &sm);
}

/// The declaration whose location is `at` (`line:col`) and that is no
/// class's injected name.
fn decl_at(ast: &kiln_ast::Ast, sm: &SourceManager, at: (u32, u32)) -> kiln_ast::DeclId {
    ast.decls
        .ids()
        .find(|&d| {
            let lc = sm.line_col(sm.spelling_loc(ast.decls.get(d).loc()));
            lc.is_some_and(|lc| (lc.line, lc.col) == at)
        })
        .unwrap_or_else(|| panic!("no declaration at {at:?}"))
}

/// What a root slot's argument list reaches (spec/ast.md §18.14.10): its
/// expressions, depth first, through alias template-ids (their written
/// arguments), `decltype`, type sugar and every subexpression.
fn root_exprs(
    ast: &kiln_ast::Ast,
    slot: kiln_ast::written_refs::RootSlot,
) -> Vec<kiln_ast::ExprId> {
    use kiln_ast::types::{ArgView, TypeView};
    let mut types = Vec::new();
    let mut exprs = Vec::new();
    let mut lists = vec![slot.as_args().expect("an argument list")];
    let mut out = Vec::new();
    while !(lists.is_empty() && types.is_empty() && exprs.is_empty()) {
        if let Some(l) = lists.pop() {
            for a in ast.types.args(l) {
                match a.view() {
                    ArgView::Type(q) => types.push(q),
                    ArgView::Expr { expr, .. } => exprs.push(expr),
                    ArgView::Pack(l) => lists.push(l),
                    _ => {}
                }
            }
        } else if let Some(q) = types.pop() {
            match ast.types.view(q.ty()) {
                TypeView::TemplateSpecialization { args, .. } => lists.push(args),
                TypeView::Decltype { expr, .. } => exprs.push(expr),
                TypeView::Typedef { underlying, .. } | TypeView::Using { underlying, .. } => {
                    types.push(underlying);
                }
                TypeView::Elaborated { named, .. } => types.push(named),
                TypeView::Paren(t)
                | TypeView::Pointer(t)
                | TypeView::LValueRef(t)
                | TypeView::RValueRef(t) => types.push(t),
                _ => {}
            }
        } else if let Some(e) = exprs.pop() {
            out.push(e);
            exprs.extend(ast.exprs.children(e, &ast.lists));
        }
    }
    out
}

/// The declarations the root slot reaches: referenced (a callee, an
/// operator function) and constructed (a constructor).
fn root_decls(
    ast: &kiln_ast::Ast,
    slot: kiln_ast::written_refs::RootSlot,
) -> Vec<kiln_ast::DeclId> {
    use kiln_ast::expr::ExprKind as K;
    root_exprs(ast, slot)
        .into_iter()
        .filter_map(|e| match ast.exprs.get(e).kind() {
            K::DeclRefExpr | K::MemberExpr => Some(ast.exprs.decl(e)),
            K::CXXConstructExpr | K::CXXTemporaryObjectExpr => Some(ast.exprs.constructor(e)),
            _ => None,
        })
        .collect()
}

/// No unresolved lookup and no dependent node among what `slot` reaches
/// (§18.14.10: names and calls in a root are resolved).
fn assert_root_resolved(ast: &kiln_ast::Ast, slot: kiln_ast::written_refs::RootSlot) {
    use kiln_ast::expr::ExprKind as K;
    for e in root_exprs(ast, slot) {
        let k = ast.exprs.get(e).kind();
        assert!(
            !matches!(
                k,
                K::UnresolvedLookupExpr
                    | K::UnresolvedMemberExpr
                    | K::DependentScopeDeclRefExpr
                    | K::CXXDependentScopeMemberExpr
                    | K::CXXUnresolvedConstructExpr
            ),
            "an unresolved node in a root: {k:?}"
        );
    }
}

const PARTIAL_ADL: &str = "namespace n { struct S {}; void swap(S &, S &); struct U {}; }\n\
                           template <class...> using void_t = void;\n\
                           template <class T> T &ref();\n\
                           template <class T, class = void> struct can_swap { static const bool value = false; };\n\
                           template <class T> struct can_swap<T, void_t<decltype(swap(ref<T>(), ref<T>()))>> { static const bool value = true; };\n\
                           bool s = can_swap<n::S>::value;\n\
                           bool u = can_swap<n::U>::value;\n";

/// Substitution keys, stage 1 (spec/ast.md §18.14.1, §18.14.10): a
/// partial specialization match through the API. `n::swap` is named by
/// one keyed row, the `swap` of the partial's arguments; its key is the
/// partial specialization of `can_swap` matched by `<n::S, void>`, with
/// the deduced `<n::S>`; the rows are in source order of the names; the
/// root has one slot per written argument, and walking slot 1 (the alias
/// template-id, its written argument, the `decltype`, its operand)
/// reaches the call of `n::swap`, every name resolved. `can_swap<n::U>`
/// has no key. The uses in the other order give the same.
#[test]
fn partial_match_keys_through_the_api() {
    let swapped = PARTIAL_ADL.replace(
        "bool s = can_swap<n::S>::value;\nbool u = can_swap<n::U>::value;\n",
        "bool u = can_swap<n::U>::value;\nbool s = can_swap<n::S>::value;\n",
    );
    assert_ne!(swapped, PARTIAL_ADL);
    for src in [PARTIAL_ADL, &swapped] {
        with_keys(src, |ast, sm| {
            let t = &ast.written_refs;
            let swap = decl_at(ast, sm, (1, 33));
            let named: Vec<_> = t.subst_rows_naming(swap).collect();
            let [(key, row)] = named[..] else { panic!("one row naming n::swap: {named:?}") };
            assert_eq!(line_col(sm, t.get(row.entry).range.lo), (5, 55));
            let k = t.subst_key(key);
            assert_eq!(k.kind, kiln_ast::written_refs::SubstKind::PartialMatch);
            let (_, info) = ast.partial_spec_info(k.template).expect("a partial specialization");
            assert_eq!(line_col(sm, ast.decls.get(info.template).loc()), (4, 41));
            let name = |q: kiln_ast::QualType| {
                use kiln_ast::types::{PrintOptions, print_type};
                print_type(&ast.types, ast, q, PrintOptions::SUGARED)
            };
            let args: Vec<String> = ast
                .types
                .args(k.args)
                .iter()
                .map(|a| match a.view() {
                    kiln_ast::types::ArgView::Type(q) => name(q),
                    v => format!("{v:?}"),
                })
                .collect();
            assert_eq!(args, ["S", "void"]);
            let first = match ast.types.args(k.args)[0].view() {
                kiln_ast::types::ArgView::Type(q) => ast.types.view(q.ty()),
                v => panic!("a type: {v:?}"),
            };
            let s = decl_at(ast, sm, (1, 22));
            assert!(matches!(first, kiln_ast::types::TypeView::Record(d) if d == s), "{first:?}");
            let deduced = k.deduced().expect("the deduced arguments");
            assert_eq!(ast.types.args(deduced).len(), 1);
            // Rows in the order the substitution reached the names
            // (§18.14.4): the two `ref`s, then `swap`, resolved once its
            // arguments are.
            let cols: Vec<(u32, u32)> =
                t.subst_rows(key).iter().map(|r| line_col(sm, t.get(r.entry).range.lo)).collect();
            assert_eq!(cols, [(5, 60), (5, 70), (5, 55)]);
            // The root: one slot per written argument.
            let root = t.subst_root(key);
            assert_eq!(root.len(), 2);
            assert!(root_decls(ast, root[1]).contains(&swap), "slot 1 reaches n::swap");
            for &slot in root {
                assert_root_resolved(ast, slot);
            }
            assert_eq!(t.subst_key_len(), 1, "no key for can_swap<n::U>");
        });
    }
}

/// §18.13, §18.14.10: an operator and a constructor no written name
/// selects are reached from the root: `a::operator<<` from the operator
/// call, `K::K(int)` from the construction.
#[test]
fn partial_match_roots_reach_operators_and_constructors() {
    let oper = "namespace a { struct X {}; int operator<<(X, X); }\n\
                template <class...> using void_t = void;\n\
                template <class T> T val();\n\
                template <class T, class = void> struct shl { static const int v = 0; };\n\
                template <class T> struct shl<T, void_t<decltype(val<T>() << val<T>())>> { static const int v = 1; };\n\
                int u = shl<a::X>::v;\n";
    with_keys(oper, |ast, sm| {
        let t = &ast.written_refs;
        let op = decl_at(ast, sm, (1, 32));
        let (key, _) = t.subst_keys().next().expect("a key");
        assert!(t.subst_rows_naming(op).any(|(k, _)| k == key), "the `<<` token's row");
        assert!(root_decls(ast, t.subst_root(key)[1]).contains(&op));
        assert_root_resolved(ast, t.subst_root(key)[1]);
    });
    let ctor = "struct K { K(int); };\n\
                template <class T, class = void> struct C {};\n\
                template <class T> struct C<T, decltype(void(T(1)))> {};\n\
                C<K> c;\n";
    with_keys(ctor, |ast, sm| {
        let t = &ast.written_refs;
        let k = decl_at(ast, sm, (1, 12));
        let (key, _) = t.subst_keys().next().expect("a key");
        assert!(t.subst_rows(key).is_empty());
        assert!(root_decls(ast, t.subst_root(key)[1]).contains(&k), "the constructor");
    });
}

/// §18.14.10: a match triggered inside a template definition with
/// concrete arguments (the body never instantiated) has a resolved root;
/// a generic lambda's closure type is a valid declaration reachable from
/// the root.
#[test]
fn partial_match_roots_in_templates_and_lambdas() {
    let in_template = "namespace n { struct S {}; void swap(S &, S &); }\n\
                       template <class T> T &ref();\n\
                       template <class T, class = void> struct can_swap { static const bool value = false; };\n\
                       template <class T> struct can_swap<T, decltype(swap(ref<T>(), ref<T>()))> { static const bool value = true; };\n\
                       template <class T> bool body(T) { return can_swap<n::S>::value; }\n";
    with_keys(in_template, |ast, sm| {
        let t = &ast.written_refs;
        let swap = decl_at(ast, sm, (1, 33));
        let (key, _) = t.subst_keys().next().expect("a key");
        assert!(t.subst_rows_naming(swap).any(|(k, r)| k == key && r.adl.is_some()));
        assert!(root_decls(ast, t.subst_root(key)[1]).contains(&swap));
        for &slot in t.subst_root(key) {
            assert_root_resolved(ast, slot);
        }
    });
    let lambda = "template <class T, class = void> struct G { static const int v = 0; };\n\
                  template <class T> struct G<T, decltype(void(T()), void([](auto x) { return x.f(T()); }))> { static const int v = 1; };\n\
                  struct H {};\n\
                  int g = G<H>::v;\n";
    with_keys(lambda, |ast, _| {
        use kiln_ast::expr::ExprKind as K;
        let t = &ast.written_refs;
        let (key, _) = t.subst_keys().next().expect("a key");
        let row = t.subst_rows(key)[0];
        assert!(row.flags & kiln_ast::written_refs::SUBST_DEPENDENT != 0);
        let lambdas: Vec<_> = root_exprs(ast, t.subst_root(key)[1])
            .into_iter()
            .filter(|&e| ast.exprs.get(e).kind() == K::LambdaExpr)
            .collect();
        let [l] = lambdas[..] else { panic!("one lambda: {lambdas:?}") };
        let closure = match ast.types.view(ast.exprs.get(l).ty().ty()) {
            kiln_ast::types::TypeView::Record(d) => d,
            v => panic!("a closure type: {v:?}"),
        };
        assert!(ast.closure_lambda(closure).is_some(), "a valid closure");
    });
}

const NAMING_CLASS_SRC: &str = concat!(
    "namespace n {\n",
    "template <class T> struct S { static int s(); using type = T; };\n",
    "}\n",
    "template <class B> struct Der : B {};\n",
    "struct FwdDer;\n",
    "struct FwdDer : n::S<int> {};\n",
    "template <class T> using A = decltype(T::s());\n",
    "template <class T> auto call() -> decltype(T::s());\n",
    "template <class T> using Ty = typename T::type;\n",
    "struct LeafBase { int g() const; int v; };\n",
    "struct LeafDer : LeafBase {};\n",
    "struct MidBase {\n",
    "  int f() const;\n",
    "  int f(int) const;\n",
    "  int x;\n",
    "  int operator+(int) const;\n",
    "  LeafDer a;\n",
    "  LeafDer b;\n",
    "};\n",
    "struct MidDer : MidBase {};\n",
    "struct SmartBase { MidBase *operator->() const; };\n",
    "struct SmartDer : SmartBase {};\n",
    "struct AllocBase { static void *operator new(decltype(sizeof(0))); };\n",
    "struct AllocDer : AllocBase {};\n",
    "namespace adl_ns { struct Tag {}; int operator-(Tag, int); void ping(Tag); }\n",
    "template <class T> int member_use(const T &t) {\n",
    "  return t.f() + t.f(1) + t.x + (t + 1) + t.a.g() + t.a.v;\n",
    "}\n",
    "template <class T> struct Host : T {\n",
    "  int run() const { return this->f() + this->x + this->b.g(); }\n",
    "};\n",
    "template <class T> int arrow_use(const T &p) { return p->x; }\n",
    "template <class T> T *alloc_use() { return new T; }\n",
    "template <class T> int non_member_use(T v) { ping(v); return v - 1; }\n",
    "template <class T> concept HasS = requires(const T &t) { T::s(); t.f(); };\n",
    "struct Both : n::S<int>, MidBase {};\n",
    "static_assert(HasS<Both>);\n",
    "template <class T, class = void> struct P { static const int v = 0; };\n",
    "template <class T> struct P<T, decltype(void(T::s()))> { static const int v = 1; };\n",
    "A<Der<n::S<int>>> a1 = 0;\n",
    "A<FwdDer> a2 = 0;\n",
    "A<n::S<int>> a3 = 0;\n",
    "Ty<Der<n::S<int>>> t1 = 0;\n",
    "int c1 = call<Der<n::S<int>>>();\n",
    "int c2 = call<n::S<int>>();\n",
    "int m1 = member_use(MidDer{});\n",
    "int m2 = member_use(MidBase{});\n",
    "int h1 = Host<MidDer>{}.run();\n",
    "int ar1 = arrow_use(SmartDer{});\n",
    "AllocDer *al1 = alloc_use<AllocDer>();\n",
    "int nm1 = non_member_use(adl_ns::Tag{});\n",
    "int p1 = P<Der<n::S<int>>>::v + P<n::S<int>>::v;\n",
);

/// spec/ast.md §18.9.1, §18.14.2 (WP-3695): `SpecRef::naming_class` and
/// `SubstRef::naming_class` carry the canonical lookup-context class for
/// resolved `Member` and `Qualified` class lookups (including when it equals
/// the entity's semantic parent class, where the dump omits `naming=`), keep
/// distinct lookup classes as distinct rows on alias templates, preserve
/// distinct naming classes across nested member chains, cover member/class
/// operators, and stay `None` for non-class, ADL, built-in and unresolved rows.
#[test]
fn spec_and_subst_rows_carry_naming_class() {
    with_keys(NAMING_CLASS_SRC, |ast, sm| {
        let t = &ast.written_refs;
        let cls_name = |d: kiln_ast::DeclId| -> String {
            use kiln_ast::types::TypeNames as _;
            let names = kiln_ast::diag_names::DiagNames::new(ast, Some(sm), 0).canonical_args();
            let mut s = String::new();
            names.write_decl_name(d, true, &mut s);
            if let Some((_, info)) = ast.spec_info(d) {
                names.write_spec_args(&names, d, info.print_args, &mut s);
            }
            s
        };
        let naming_of = |at: (u32, u32)| -> Vec<Option<String>> {
            let w = entry_at(t, sm, at);
            let mut v: Vec<Option<String>> =
                t.specs_of(w).map(|(_, r)| r.naming_class.map(cls_name)).collect();
            v.sort();
            v
        };
        // Forward-declared FwdDer: naming_class must be the canonical first decl (line 5).
        let fwd_first = ast.first(decl_at(ast, sm, (5, 8)));
        let a_entry = entry_at(t, sm, (7, 42));
        let a_rows: Vec<_> = t.specs_of(a_entry).map(|(_, r)| r.naming_class).collect();
        assert_eq!(a_rows.len(), 3, "distinct naming classes stay distinct rows on alias A");
        assert!(a_rows.contains(&Some(fwd_first)), "canonical first decl for FwdDer");
        assert_eq!(
            naming_of((7, 42)),
            vec![Some("Der<n::S<int>>".into()), Some("FwdDer".into()), Some("n::S<int>".into()),]
        );
        // Trailing decltype(T::s()) signature: both derived and direct carry naming_class.
        assert_eq!(
            naming_of((8, 47)),
            vec![Some("Der<n::S<int>>".into()), Some("n::S<int>".into())]
        );
        // Dependent typename T::type.
        assert_eq!(naming_of((9, 43)), vec![Some("Der<n::S<int>>".into())]);
        // Member calls, fields, operator+, and nested chains in member_use.
        let base_and_der = vec![Some("MidBase".into()), Some("MidDer".into())];
        assert_eq!(naming_of((27, 12)), base_and_der); // t.f()
        assert_eq!(naming_of((27, 20)), base_and_der); // t.f(1)
        assert_eq!(naming_of((27, 29)), base_and_der); // t.x
        assert_eq!(naming_of((27, 36)), base_and_der); // t + 1 (member operator+)
        assert_eq!(naming_of((27, 45)), base_and_der); // t.a (in t.a.g())
        assert_eq!(naming_of((27, 47)), vec![Some("LeafDer".into()), Some("LeafDer".into())]); // .g()
        assert_eq!(naming_of((27, 55)), base_and_der); // t.a (in t.a.v)
        assert_eq!(naming_of((27, 57)), vec![Some("LeafDer".into()), Some("LeafDer".into())]); // .v
        // Built-in `+` rows get None.
        assert_eq!(naming_of((27, 16)), vec![None, None]);
        // Implicit/explicit `this->` and nested chain `this->b.g()`.
        assert_eq!(naming_of((30, 34)), vec![Some("Host<MidDer>".into())]); // this->f()
        assert_eq!(naming_of((30, 46)), vec![Some("Host<MidDer>".into())]); // this->x
        assert_eq!(naming_of((30, 56)), vec![Some("Host<MidDer>".into())]); // this->b
        assert_eq!(naming_of((30, 58)), vec![Some("LeafDer".into())]); // .g()
        // Member operator-> and class-scope operator new.
        assert_eq!(naming_of((32, 56)), vec![Some("SmartDer".into())]); // ->
        assert_eq!(naming_of((32, 58)), vec![Some("MidBase".into())]); // x
        assert_eq!(naming_of((33, 44)), vec![Some("AllocDer".into())]); // new
        // Non-member ADL call and non-member ADL operator- get None.
        assert_eq!(naming_of((34, 46)), vec![None]); // ping(v)
        assert_eq!(naming_of((34, 64)), vec![None]); // v - 1
        // Concept requires-clause.
        assert_eq!(naming_of((35, 61)), vec![Some("Both".into())]); // T::s()
        assert_eq!(naming_of((35, 68)), vec![Some("Both".into())]); // t.f()
        // Keyed rows (`SubstRef::naming_class`) for partial specialization matches.
        let p_entry = entry_at(t, sm, (39, 49));
        let mut p_naming: Vec<Option<String>> =
            t.subst_rows_of(p_entry).map(|(_, r)| r.naming_class.map(cls_name)).collect();
        p_naming.sort();
        assert_eq!(p_naming, vec![Some("Der<n::S<int>>".into()), Some("n::S<int>".into())]);
    });
}

/// WP-3688: label uses (`goto`, `&&`, `asm goto`) record `RefRole::Label`
/// entries pointing to their `LabelDecl`, with no per-specialization/keyed
/// rows in templates, and `AstNames::decl_key` gives every label (including
/// multiple `__label__` declarations of the same name in one function) a
/// distinct, stable key.
#[test]
fn label_uses_and_decl_keys() {
    let src = "\
void g() { f: ; goto f; void *p = &&f; (void)p; }
void h(int x) { asm goto (\"\" : : \"r\"(x) : : out); out: ; }
void k() {
  { __label__ m; goto m; m: ; }
  { __label__ m; goto m; m: ; }
}
template <class T> void ft(T x) { l: ; void *p = &&l; (void)p; (void)x; }
void use_ft() { ft(1); ft(2.0); }
";
    with_keys(src, |ast, sm| {
        let t = &ast.written_refs;
        let names = kiln_ast::dump::AstNames::with_sources(ast, sm);
        let label_at =
            |at: (u32, u32)| -> (kiln_ast::written_refs::WrittenRefId, kiln_ast::DeclId) {
                let id = entry_at(t, sm, at);
                let r = t.get(id);
                assert_eq!(r.role, RefRole::Label, "at {at:?}");
                assert_eq!(r.how, kiln_ast::written_refs::RefLookup::Unqualified, "at {at:?}");
                (id, r.entity.expect("label entity"))
            };
        let (_, f1) = label_at((1, 22));
        let (_, f2) = label_at((1, 37));
        assert_eq!(f1, f2);
        assert_eq!(names.decl_key(f1), "g()::f");

        let (_, out) = label_at((2, 45));
        assert_eq!(names.decl_key(out), "h(int)::out");

        let (_, m1) = label_at((4, 23));
        let (_, m2) = label_at((5, 23));
        assert_ne!(m1, m2, "distinct __label__ decls");
        assert_eq!(names.decl_key(m1), "k()::m");
        assert_eq!(names.decl_key(m2), "k()::m#2");

        let (l_id, l_decl) = label_at((7, 52));
        assert_eq!(names.decl_key(l_decl), "ft(type-parameter-0-0)::l");
        assert_eq!(t.specs_of(l_id).count(), 0, "no SpecRef rows for pattern-local label");
        assert_eq!(t.subst_rows_of(l_id).count(), 0, "no SubstRef rows for pattern-local label");
    });
}
