//! Source-driven tests of sema rules that need a parser in front of them:
//! enumeration underlying types, aggregate constant values, local externs
//! and odr-use marking.

mod common;
use common::parse;

/// [dcl.enum]/7: an unfixed enum's underlying type fits all enumerators.
#[test]
fn dcl_enum_best_type() {
    let p = parse(
        "enum A { a = -1 }; enum B { b = 0xffffffffu }; enum C { c = 0x100000000 }; \
         static_assert(sizeof(A) == 4 && sizeof(B) == 4 && sizeof(C) == 8); \
         static_assert((A)-1 < 0 && (B)-1 > 0);",
    );
    p.assert_clean();
}

/// [dcl.enum]/5: enumerator types inside and after the definition.
#[test]
fn dcl_enum_enumerator_types() {
    let p =
        parse("enum E : unsigned char { x = 200, y }; static_assert(y == 201 && sizeof(y) == 1);");
    p.assert_clean();
    let p = parse("enum E : unsigned char { x = 255, y };");
    p.assert_diag(
        "enumerator value 256 is not representable in the underlying type 'unsigned char'",
    );
}

/// [expr.const]: members of constexpr aggregates are usable in constant expressions.
#[test]
fn expr_const_aggregate_members() {
    let p = parse(
        "struct P { int x, y; }; constexpr P p = {3, 4}; constexpr int a[] = {1, 2, 3}; \
         static_assert(p.x + p.y == 7 && a[2] == 3 && sizeof(a) == 12);",
    );
    p.assert_clean();
}

/// [dcl.constexpr]/10: a constexpr variable needs a constant initializer.
#[test]
fn dcl_constexpr_needs_constant_init() {
    let p = parse("int g(); constexpr int k = g();");
    p.assert_diag("constexpr variable 'k' must be initialized by a constant expression");
}

/// [basic.link]/7: a block-scope extern names the namespace-scope entity.
#[test]
fn basic_link_local_extern() {
    let p = parse("void f() { extern int n; n = 1; } int n = 0;");
    p.assert_clean();
    let p = parse("void f() { extern int n; } long n;");
    p.assert_diag("1:33 error: redefinition of 'n' with a different type: 'long' vs 'int'");
    p.assert_diag("1:23 note: previous declaration is here");
    let p = parse("void f() { extern int n; } extern long n;");
    p.assert_diag("redeclaration of 'n' with a different type: 'long' vs 'int'");
}

/// [basic.def.odr]/4: sizeof operands and constant references are not odr-uses.
#[test]
fn basic_def_odr_use() {
    let p = parse("int g; constexpr int k = 4; int f() { return sizeof(g) + k; }");
    p.assert_clean();
    // Referenced but not `used`: neither name is odr-used.
    p.assert_dump("VarDecl referenced g 'int'\n");
    p.assert_dump("VarDecl referenced k 'const int' constexpr cinit value=4");
    p.assert_dump("DeclRefExpr 'const int' lvalue Var");
    p.assert_dump("non_odr_use_constant");
    p.assert_dump("non_odr_use_unevaluated");
}

/// [stmt.pre]/4, [basic.scope.block]/2: a condition variable is in scope in both
/// branches, and the outermost block of a sub-statement may not redeclare it.
#[test]
fn stmt_pre_condition_scope() {
    let p = parse("int f(int a) { if (int x = a) return x; else return x + 1; }");
    p.assert_clean();
    let p = parse("int f(int a) { if (int x = a) { int x = 2; return x; } return 0; }");
    p.assert_diag("1:37 error: redefinition of 'x'");
    p.assert_diag("1:24 note: previous definition is here");
    let p = parse("void f(int a) { for (int i = a; i; --i) { int i = 0; } }");
    p.assert_diag("redefinition of 'i'");
    // A nested block may shadow it.
    let p = parse("int f(int a) { while (int x = a) { { int x = 0; } return x; } return 0; }");
    p.assert_clean();
}
