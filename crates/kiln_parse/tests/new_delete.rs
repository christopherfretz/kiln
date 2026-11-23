//! Per-rule tests for new- and delete-expressions ([expr.new],
//! [expr.delete]; spec/parse.md §5.1).

mod common;
use common::parse;

/// [expr.new]: a new-type-id with no initializer, `()`, `(args)` and `{}`.
#[test]
fn new_initializer_forms() {
    let p = parse(
        "int *a() { return new int; } int *b() { return new int(3); } int *c() { return new int{4}; } int *d() { return new int(); }",
    );
    p.assert_clean();
    p.assert_dump("CXXNewExpr 'int *' Function 'operator new' 'void *(unsigned long)'\n");
    p.assert_dump("`-ImplicitValueInitExpr 'int'\n");
    p.assert_dump("`-InitListExpr 'int'\n");
}

/// [expr.new]: `::new` selects the global operator and is dumped `global`.
#[test]
fn new_global() {
    let p = parse("long *f() { return ::new long{}; }");
    p.assert_clean();
    p.assert_dump("CXXNewExpr 'long *' global Function 'operator new'");
}

/// [expr.new]: a parenthesized type-id, `new (int *)`, is not a placement.
#[test]
fn new_paren_type_id() {
    let p = parse("int **f() { return new (int *); } int (**g())[2] { return new (int (*)[2]); }");
    p.assert_clean();
    p.assert_dump("CXXNewExpr 'int **' Function 'operator new' 'void *(unsigned long)'\n");
    p.assert_dump("CXXNewExpr 'int (**)[2]'");
}

/// [expr.new]: `new (short(n), 'x') int` is a placement, because the
/// `(` after `short` cannot start an abstract declarator.
#[test]
fn new_placement_disambiguation() {
    let p = parse(
        "void *operator new(unsigned long, short, char) noexcept;\n\
         int *f(int n) { return new (short(n), 'x') int(1); }",
    );
    p.assert_clean();
    p.assert_dump(
        "CXXNewExpr 'int *' Function 'operator new' 'void *(unsigned long, short, char) noexcept'",
    );
}

/// [expr.new]: placement followed by a parenthesized type-id.
#[test]
fn new_placement_then_paren_type_id() {
    let p = parse(
        "inline void *operator new(unsigned long, void *p) noexcept { return p; }\n\
         int *f(void *b) { return new (b) (int)(5); }",
    );
    p.assert_clean();
    p.assert_dump(
        "CXXNewExpr 'int *' Function 'operator new' 'void *(unsigned long, void *) noexcept'",
    );
}

/// [expr.new]: array new; only the first bound may be non-constant, and
/// the result points to the first element.
#[test]
fn new_array_declarators() {
    let p = parse(
        "int (*f(long n))[3] { return new int[n][3]; } int **g(int n) { return new int *[n]; }",
    );
    p.assert_clean();
    p.assert_dump("CXXNewExpr 'int (*)[3]' array Function 'operator new[]'");
    p.assert_dump("CXXNewExpr 'int **' array Function 'operator new[]'");
}

/// [expr.new]: a new-expression cannot be followed by a postfix operator
/// (clang's `AllowSuffix = false`).
#[test]
fn new_postfix_requires_parens() {
    let p = parse("int f() { return new int(1)[0]; }");
    p.assert_diag(
        "1:28 error: expression cannot be followed by a postfix '[' operator; add parentheses",
    );
}

/// [expr.new]: `new auto(x)` deduces the allocated type.
#[test]
fn new_auto() {
    let p = parse("double *f() { return new auto(2.5); }");
    p.assert_clean();
    p.assert_dump("CXXNewExpr 'double *' Function 'operator new' 'void *(unsigned long)'");
}

/// [expr.delete]: `delete`, `delete[]`, `::delete` and `::delete[]`.
#[test]
fn delete_forms() {
    let p = parse(
        "void a(int *p) { delete p; } void b(int *p) { delete[] p; }\n\
         void c(int *p) { ::delete p; } void d(const double *q) { ::delete[] q; }",
    );
    p.assert_clean();
    p.assert_dump(
        "CXXDeleteExpr 'void' Function 'operator delete' 'void (void *, unsigned long) noexcept'",
    );
    p.assert_dump(
        "CXXDeleteExpr 'void' array Function 'operator delete[]' 'void (void *) noexcept'",
    );
    p.assert_dump("CXXDeleteExpr 'void' global Function 'operator delete'");
    p.assert_dump("CXXDeleteExpr 'void' global array Function 'operator delete[]'");
}

/// [expr.delete]: the operand must be a pointer to an object type.
#[test]
fn delete_operand_errors() {
    let p = parse("void f(int i) { delete i; }");
    p.assert_diag("1:17 error: cannot delete expression of type 'int'");
    let p = parse("void f(void *v) { delete v; }");
    p.assert_diag("1:19 error: cannot delete pointer to incomplete type 'void'");
}

/// The global allocation and deallocation functions a program declares,
/// as entities: for each name and type, the distinct first declarations of
/// the non-pattern declarations (a friend declared in a class template's
/// pattern declares nothing until instantiated, [temp.friend]/1); and the
/// first declaration of each new- and delete-expression's function, with
/// whether it is such a pattern friend. Asserts the source is clean.
fn alloc_entities(src: &str) -> (Vec<Vec<u32>>, Vec<(u32, bool)>) {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use kiln_ast::decl::DeclKind;
    use kiln_ast::dump::{DumpOptions, dump};
    use kiln_ast::names::{DeclName, OverloadedOperator as O};
    use kiln_ast::{DeclId, RawId};
    use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
    use kiln_sema::target::{Arch, Target};
    use kiln_source::SourceManager;
    use kiln_source::symbol::Interner;

    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let opts = common::opts19();
    let dopts = DiagOptions { clang_compat: opts.clang_compat, ..DiagOptions::default() };
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &dopts, sink.clone());
    let ast = kiln_parse::parse_file(
        &sm,
        &interner,
        file,
        &engine,
        Target::for_arch(Arch::X86_64),
        &opts,
    );
    assert!(sink.take().is_empty(), "unexpected diagnostics");
    let names: Vec<DeclName> = [O::New, O::ArrayNew, O::Delete, O::ArrayDelete]
        .into_iter()
        .map(DeclName::operator)
        .collect();
    let pattern_friend = |d: DeclId| {
        ast.decls.lexical_ctx(d).is_some_and(|l| {
            ast.decls.get(l).kind() == DeclKind::CXXRecord && ast.described_template(l).is_some()
        })
    };
    let mut groups: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for d in (1..=ast.decls.len() as u32).map(DeclId::new) {
        let n = ast.decls.get(d);
        if n.kind() != DeclKind::Function || !names.contains(&n.name()) || pattern_friend(d) {
            continue;
        }
        let Some(ty) = n.ty() else { continue };
        let key = format!("{:?} {:?}", n.name(), ast.types.canonical(ty));
        let first = ast.first(d).raw();
        let g = groups.entry(key).or_default();
        if !g.contains(&first) {
            g.push(first);
        }
    }
    let opts = DumpOptions { ids: true, implicit: true, ..DumpOptions::default() };
    let mut buf = Vec::new();
    dump(&ast, &opts, &mut buf).expect("dump to memory");
    let text = String::from_utf8(buf).expect("utf-8 dump");
    let mut callees = Vec::new();
    for line in text.lines().filter(|l| l.contains("CXXNewExpr") || l.contains("CXXDeleteExpr")) {
        let Some((_, rest)) = line.split_once(" Function #d") else { continue };
        let id: u32 = rest.split(' ').next().and_then(|s| s.parse().ok()).expect("callee id");
        let d = DeclId::new(id);
        callees.push((ast.first(d).raw(), pattern_friend(d)));
    }
    (groups.into_values().collect(), callees)
}

/// Asserts that each global allocation function of `src` is one entity
/// (one redeclaration chain) and that every new- and delete-expression
/// calls one of them, never a pattern's friend.
#[track_caller]
fn assert_one_entity_each(src: &str, expressions: usize) {
    let (groups, callees) = alloc_entities(src);
    for g in &groups {
        assert_eq!(g.len(), 1, "one function declared as {} entities", g.len());
    }
    assert_eq!(callees.len(), expressions, "new- and delete-expressions found");
    for (first, pattern) in callees {
        assert!(!pattern, "an expression calls a pattern's friend");
        assert!(groups.iter().any(|g| g[0] == first), "callee #d{first} is no global function");
    }
}

/// [temp.friend]/1, [basic.stc.dynamic.general]/2, [basic.scope.scope]/4
/// (WP-2540): a global `operator new`/`delete` declared after a class
/// template whose pattern befriends it redeclares the implicit declaration
/// and the friend the instantiation declared, not the pattern's friend; so
/// does one declared before the instantiation. Plain, array, aligned and
/// placement forms; the expressions call the global functions.
#[test]
fn pattern_friend_is_not_the_global_alloc_function() {
    let forms = "friend void *operator new(std::size_t);\n\
                 friend void operator delete(void *) noexcept;\n\
                 friend void *operator new[](std::size_t);\n\
                 friend void operator delete[](void *) noexcept;\n\
                 friend void *operator new(std::size_t, std::align_val_t);\n\
                 friend void operator delete(void *, std::align_val_t) noexcept;\n\
                 friend void *operator new(std::size_t, void *) noexcept;\n\
                 friend void operator delete(void *, void *) noexcept;\n";
    let globals = forms.replace("friend ", "");
    let head = "namespace std { using size_t = decltype(sizeof 0); \
                enum class align_val_t : size_t {}; }\n\
                struct alignas(64) O { int x; };\n";
    let uses = "void use(void *buf) { int *p = new int; delete p; int *q = new int[2]; \
                delete[] q; O *o = new O; delete o; int *r = new (buf) int; (void)r; }\n";
    // The instantiation first, then the global declarations.
    let after =
        format!("{head}template <class T> struct A {{\n{forms}}};\nA<int> a;\n{globals}{uses}");
    assert_one_entity_each(&after, 7);
    // The global declarations first, then the instantiation.
    let before =
        format!("{head}template <class T> struct A {{\n{forms}}};\n{globals}A<int> a;\n{uses}");
    assert_one_entity_each(&before, 7);
    // No global declaration: the friends redeclare the implicit functions,
    // which the expressions call (no placement form: only ADL finds the
    // friend `operator new(size_t, void *)`, and nothing else declares it).
    let uses = uses.replace("int *r = new (buf) int; (void)r; ", "(void)buf; ");
    let none = format!("{head}template <class T> struct A {{\n{forms}}};\nA<int> a;\n{uses}");
    let (groups, callees) = alloc_entities(&none);
    assert!(groups.iter().all(|g| g.len() == 1), "a friend is a second entity");
    assert!(callees.iter().all(|c| !c.1), "an expression calls a pattern's friend");
    // Not instantiated: the pattern's friend is not what `new` finds.
    let uninst = format!("{head}template <class T> struct A {{\n{forms}}};\n{uses}");
    let (_, callees) = alloc_entities(&uninst);
    assert!(callees.iter().all(|c| !c.1), "an expression calls a pattern's friend");
}

/// [class.friend]/11, [basic.stc.dynamic.general]/2 (WP-2540): a friend of
/// a non-template class naming a global `operator new` or `operator
/// new[]`, unqualified or `::`-qualified, redeclares the implicit function
/// (the qualified one was "does not match any declaration").
#[test]
fn friend_redeclares_the_implicit_alloc_function() {
    assert_one_entity_each(
        "struct B { friend void *operator new(decltype(sizeof 0)); };\n\
         struct C { friend void *::operator new[](decltype(sizeof 0)); };\n\
         void *operator new(decltype(sizeof 0));\n\
         int *f() { return new int[2]; }",
        1,
    );
}
