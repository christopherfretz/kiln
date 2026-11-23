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
