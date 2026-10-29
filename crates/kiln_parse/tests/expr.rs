//! Per-rule tests for expressions ([expr], [lex.literal]).

mod common;
use common::parse;

/// [expr.pre]: precedence and associativity of the binary operators.
#[test]
fn expr_precedence() {
    let p = parse("int f(int a, int b, int c) { return a + b * c - a / b; }");
    p.assert_clean();
    p.assert_dump("BinaryOperator 'int' '-'\n");
    let d = &p.dump;
    let minus = d.find("'-'").unwrap();
    let plus = d.find("'+'").unwrap();
    let times = d.find("'*'").unwrap();
    assert!(minus < plus && plus < times, "{d}");
}

/// [expr.ass]: assignment is right-associative and needs a modifiable lvalue.
#[test]
fn expr_ass() {
    let p = parse("void f() { int a, b; a = b = 1; const int c = 0; c = 2; }");
    p.assert_diag("cannot assign to variable 'c' with const-qualified type 'const int'");
}

/// [expr.cond]: the conditional operator.
#[test]
fn expr_cond() {
    let p = parse("int f(bool b) { return b ? 1 : 2L; }");
    p.assert_clean();
    p.assert_dump("ConditionalOperator 'long'");
}

/// [expr.call], [over.match.viable]: argument-count mismatches (clang wording).
#[test]
fn expr_call_arity() {
    let p = parse("int g(int, int); int f() { return g(1); }");
    p.assert_diag("1:35 error: no matching function for call to 'g'");
    p.assert_diag("note: candidate function not viable: requires 2 arguments, but 1 was provided");
    let p = parse("int g(int); int f() { return g(1, 2); }");
    p.assert_diag("note: candidate function not viable: requires 1 argument, but 2 were provided");
}

/// [expr.call]: variadic arguments undergo default promotions.
#[test]
fn expr_call_variadic_promotion() {
    let p = parse("int printf(const char *, ...); void f(float x) { printf(\"%f\", x); }");
    p.assert_clean();
    p.assert_dump("ImplicitCastExpr 'double' <FloatingCast>");
}

/// [expr.sub]: subscripting either operand order.
#[test]
fn expr_sub() {
    let p = parse("int a[4]; int f() { return a[1] + 2[a]; }");
    p.assert_clean();
    p.assert_dump("ArraySubscriptExpr 'int' lvalue");
}

/// [expr.ref]: `.` and `->`.
#[test]
fn expr_ref() {
    let p = parse("struct P { int x; }; int f(P p, P *q) { return p.x + q->x; }");
    p.assert_clean();
    p.assert_dump("MemberExpr 'int' lvalue ->x");
    p.assert_dump("MemberExpr 'int' lvalue .x");
    let p = parse("struct P { int x; }; int f(P p) { return p.y; }");
    p.assert_diag("no member named 'y' in 'P'");
}

/// [expr.post.incr], [expr.pre.incr].
#[test]
fn expr_incr() {
    let p = parse("void f(int i) { i++; ++i; i--; --i; }");
    p.assert_clean();
    p.assert_dump("UnaryOperator 'int' postfix '++'");
    p.assert_dump("UnaryOperator 'int' lvalue prefix '--'");
}

/// [expr.sizeof]: sizeof of types and expressions; not of functions.
#[test]
fn expr_sizeof() {
    let p = parse("static_assert(sizeof(char) == 1 && sizeof(long) == 8 && sizeof(int[3]) == 12);");
    p.assert_clean();
    let p = parse("void g(); unsigned long n = sizeof(g);");
    p.assert_diag("invalid application of 'sizeof' to a function type");
}

/// [expr.sizeof]: `sizeof...` needs a pack (none exist in M2).
#[test]
fn expr_sizeof_pack() {
    let p = parse("int x; unsigned long n = sizeof...(x);");
    p.assert_diag("'x' does not refer to the name of a parameter pack");
}

/// [expr.alignof].
#[test]
fn expr_alignof() {
    let p = parse("static_assert(alignof(double) == 8 && alignof(char) == 1);");
    p.assert_clean();
}

/// [expr.cast]: C-style casts pick static/const/reinterpret.
#[test]
fn expr_cast_c_style() {
    let p = parse("long f(int *p, const int *q) { return (long)p + *(int *)q + (int)1.5; }");
    p.assert_clean();
    p.assert_dump("CStyleCastExpr 'long' <PointerToIntegral>");
    p.assert_dump("CStyleCastExpr 'int *' <NoOp>");
}

/// [expr.static.cast]: no casting away constness.
#[test]
fn expr_static_cast() {
    let p = parse("void f(const int *p) { int *q = static_cast<int *>(p); }");
    p.assert_diag("static_cast from 'const int *' to 'int *' is not allowed");
}

/// [expr.reinterpret.cast].
#[test]
fn expr_reinterpret_cast() {
    let p = parse("long f(int *p) { return reinterpret_cast<long>(p); }");
    p.assert_clean();
    p.assert_dump("CXXReinterpretCastExpr 'long' reinterpret_cast<long> <PointerToIntegral>");
}

/// [expr.const.cast].
#[test]
fn expr_const_cast() {
    let p = parse("int *f(const int *p) { return const_cast<int *>(p); }");
    p.assert_clean();
    p.assert_dump("CXXConstCastExpr 'int *' const_cast<int *> <NoOp>");
}

/// [expr.type.conv]: functional casts.
#[test]
fn expr_type_conv() {
    let p = parse("double f(int i) { return double(i); }");
    p.assert_clean();
    p.assert_dump("CXXFunctionalCastExpr 'double' functional cast to double <NoOp>");
}

/// [expr.unary.op]: address-of and indirection.
#[test]
fn expr_unary_op() {
    let p = parse("int f(int x) { int *p = &x; return *p; }");
    p.assert_clean();
    p.assert_dump("UnaryOperator 'int *' prefix '&' cannot overflow");
    let p = parse("int f(int x) { return *x; }");
    p.assert_diag("indirection requires pointer operand ('int' invalid)");
}

/// [expr.mul]: division by a constant zero warns.
#[test]
fn expr_mul_div_by_zero() {
    let p = parse("int f(int a) { return a / 0 + a % 0; }");
    p.assert_diag("warning: division by zero is undefined");
    p.assert_diag("warning: remainder by zero is undefined");
}

/// [expr.shift]: negative and oversized shift counts warn.
#[test]
fn expr_shift_counts() {
    let p = parse("int f(int a) { return (a << -1) + (a >> 40); }");
    p.assert_diag("warning: shift count is negative");
    p.assert_diag("warning: shift count >= width of type");
}

/// [expr.add]: pointer arithmetic.
#[test]
fn expr_add_pointer() {
    let p = parse("long f(int *p, int *q) { return (p + 1) - q; }");
    p.assert_clean();
    p.assert_dump("BinaryOperator 'long' '-'");
}

/// [expr.prim.id.unqual]: undeclared identifiers.
#[test]
fn expr_prim_id_undeclared() {
    let p = parse("int f() { return nope; }");
    p.assert_diag("use of undeclared identifier 'nope'");
}

/// [lex.string]: adjacent string literals concatenate.
#[test]
fn lex_string_concat() {
    let p = parse("const char *s = \"ab\" \"cd\";");
    p.assert_clean();
    p.assert_dump("StringLiteral 'const char[5]' lvalue \"abcd\"");
}

/// [lex.ext]: user-defined literals need a literal operator (none in M2).
#[test]
fn lex_ext_udl_rejected() {
    let p = parse("int x = 12_km;");
    p.assert_diag("no matching literal operator for call to 'operator\"\"_km'");
}

/// [lex.nullptr], [lex.bool].
#[test]
fn lex_nullptr_bool() {
    let p = parse("int *p = nullptr; bool b = true; bool c = !false;");
    p.assert_clean();
    p.assert_dump("CXXNullPtrLiteralExpr 'std::nullptr_t'");
    p.assert_dump("CXXBoolLiteralExpr 'bool' true");
}

/// [expr.const]: enumerators and constexpr variables fold.
#[test]
fn expr_const_folding() {
    let p = parse("enum { N = 4 }; constexpr int k = N * 2; static_assert(k == 8); int a[k];");
    p.assert_clean();
    p.assert_dump("VarDecl a 'int[8]'");
}

/// [expr.unary.noexcept]: a bool prvalue over an unevaluated operand.
#[test]
fn expr_unary_noexcept() {
    let p = parse(
        "int may(); int wont() noexcept;\n\
         static_assert(!noexcept(may())); static_assert(noexcept(wont() + 1));\n\
         static_assert(noexcept(1)); static_assert(!noexcept(may() + wont()));",
    );
    p.assert_clean();
    p.assert_dump("CXXNoexceptExpr 'bool'");
    let p = parse("bool b = noexcept 1;");
    p.assert_diag("expected '(' after 'noexcept'");
}

/// [stmt.ambig]: an ambiguous statement is a declaration when it can be;
/// a trailing comma-expression makes it an expression.
#[test]
fn stmt_ambig() {
    let p = parse("typedef int T; int x; void g() { T(a); T(*b)[2]; (void)T(x); }");
    p.assert_clean();
    p.assert_dump("VarDecl a 'T':'int'");
    p.assert_dump("VarDecl b 'T (*)[2]'");
    p.assert_dump("CXXFunctionalCastExpr");
    let p = parse("typedef int T; int y; void g() { T(y), (void)0; }");
    p.assert_clean();
    p.assert_dump("BinaryOperator 'void' ','");
}

/// [dcl.ambig.res]: `sizeof(T(1))` is an expression, `sizeof(T())` a
/// type-id (of function type, which `sizeof` rejects).
#[test]
fn dcl_ambig_res() {
    let p = parse("typedef int T; unsigned long a = sizeof(T(1));");
    p.assert_clean();
    p.assert_dump("UnaryExprOrTypeTraitExpr");
    let p = parse("typedef int T; unsigned long a = sizeof(T());");
    p.assert_diag("1:34 error: invalid application of 'sizeof' to a function type");
    let p = parse("void f() { int a(); }");
    p.assert_diag("warning: empty parentheses interpreted as a function declaration");
}

/// [expr.type.conv]: a type name must be followed by `(` or `{`.
#[test]
fn expr_type_conv_errors() {
    let p = parse("int d = int;");
    p.assert_diag("1:12 error: expected '(' for function-style cast or type construction");
    let p = parse("typedef int T; int e = T + 1;");
    p.assert_diag("1:24 error: unexpected type name 'T': expected expression");
}

/// GNU extensions: statement expressions, `__builtin_offsetof` and
/// scalar compound literals.
#[test]
fn gnu_extensions() {
    let p = parse("int f() { return ({ int a = 1; a + 1; }); }");
    p.assert_clean();
    p.assert_dump("StmtExpr");
    let p = parse("struct S { int a, b; }; unsigned long o = __builtin_offsetof(S, b);");
    p.assert_clean();
    p.assert_dump("OffsetOfExpr");
    let p = parse("int f() { return (const int){3} + 1; }");
    p.assert_clean();
    p.assert_dump("CompoundLiteralExpr");
    let p = parse("void f() { (void)(int){1, 2}; }");
    p.assert_diag("error: function-style cast to a builtin type can only take one argument");
}

/// [expr.throw] and [expr.typeid] parse as unary-like expressions.
#[test]
fn throw_and_typeid() {
    let p = parse("void f(int x) { if (x) throw x + 1; }");
    p.assert_clean();
    p.assert_dump("CXXThrowExpr");
    let p = parse("void f() { throw; }");
    p.assert_clean();
}

/// `__builtin_is_constant_evaluated` is declared on first use (clang
/// `LazilyCreateBuiltin`); its name must be called directly.
#[test]
fn builtin_is_constant_evaluated() {
    let p = parse("bool f() { return __builtin_is_constant_evaluated(); }");
    p.assert_clean();
    p.assert_dump(
        "ImplicitCastExpr 'bool (*)() noexcept' <BuiltinFnToFnPtr>\n\
         \x20         `-DeclRefExpr '<builtin fn type>' Function '__builtin_is_constant_evaluated' 'bool () noexcept'",
    );
    let p = parse(
        "auto p = &__builtin_is_constant_evaluated;\nvoid g() { __builtin_is_constant_evaluated; }",
    );
    p.assert_diag("1:11 error: builtin functions must be directly called");
    p.assert_diag("2:12 error: builtin functions must be directly called");
    let p = parse("bool (*q)() = __builtin_is_constant_evaluated;");
    p.assert_diag("1:15 error: builtin functions must be directly called");
    let p = parse("static_assert(__builtin_is_constant_evaluated());");
    p.assert_diag("1:15 warning: '__builtin_is_constant_evaluated' will always evaluate to 'true'");
}
