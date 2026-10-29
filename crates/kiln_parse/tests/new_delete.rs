//! Per-rule tests for new- and delete-expressions ([expr.new],
//! [expr.delete]; spec/parse.md §5.9).

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
