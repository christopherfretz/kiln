// [dcl.type.general]/2: a declaration whose decl-specifier-seq has no
// defining-type-specifier is ill-formed. As in clang, the error is "a type
// specifier is required for all declarations" at the declarator, and the
// name is declared as an invalid `int` (a namespace-scope `y;`, a member
// `y();`) so later uses are silent; an untyped redeclaration is not merged
// and lookup prefers the valid declaration.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:1 error "a type specifier is required for all declarations"
//@ diag: 25:8 error "a type specifier is required for all declarations"
//@ diag: 26:7 error "a type specifier is required for all declarations"
//@ diag: 29:7 error "a type specifier is required for all declarations"
//@ diag: 31:1 error "a type specifier is required for all declarations"
//@ diag: 31:5 error "expected ';' after top level declarator"
//@ diag: 32:9 error "use of undeclared identifier 'u'"
//@ diag: 34:3 error "a type specifier is required for all declarations"
//@ diag: 35:3 error "a type specifier is required for all declarations"
//@ diag: 35:6 error "a type specifier is required for all declarations"
//@ diag: 36:3 error "a type specifier is required for all declarations"
//@ diag: 37:3 error "a type specifier is required for all declarations"
y;
int a = y;
int b = y + 1;
static z;
const w = 0;
int c = w;
const int k = 1;
const k = 2;
static_assert(k == 1);
v, u = 1;
int d = u;
struct S {
  m;
  n, o;
  f();
  g(int) const;
  int h() { return f() + m + o; }
};
int i = S().f();
auto pf = &S::f;
