// clang's `require_constant_initialization` before C++11 (WP-954; clang
// `CheckCompleteVariableDeclaration` with `Expr::isConstantInitializer`,
// [basic.start.init]/2 C++03): aggregates, trivial default and copy
// construction and the no-op conversions are walked and every other
// subexpression must fold; the one that does not gets the single
// `subexpression not valid` note at its location.
//@ std: c++03
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:10 error "variable does not have a constant initializer"
//@ diag: 37:1 note "required by 'require_constant_initialization' attribute here"
//@ diag: 30:29 note "expanded from macro 'ATTR'"
//@ diag: 37:14 note "subexpression not valid in a constant expression"
//@ diag: 39:10 error "variable does not have a constant initializer"
//@ diag: 39:1 note "required by 'require_constant_initialization' attribute here"
//@ diag: 30:29 note "expanded from macro 'ATTR'"
//@ diag: 39:19 note "subexpression not valid in a constant expression"
//@ diag: 41:10 error "variable does not have a constant initializer"
//@ diag: 41:1 note "required by 'require_constant_initialization' attribute here"
//@ diag: 30:29 note "expanded from macro 'ATTR'"
//@ diag: 41:21 note "subexpression not valid in a constant expression"
//@ diag: 42:14 error "variable does not have a constant initializer"
//@ diag: 42:1 note "required by 'require_constant_initialization' attribute here"
//@ diag: 30:29 note "expanded from macro 'ATTR'"
//@ diag: 42:14 note "subexpression not valid in a constant expression"
//@ diag: 45:17 error "variable does not have a constant initializer"
//@ diag: 45:1 note "required by 'require_constant_initialization' attribute here"
//@ diag: 30:29 note "expanded from macro 'ATTR'"
//@ diag: 45:22 note "subexpression not valid in a constant expression"
#define ATTR __attribute__((require_constant_initialization))
int f() { return 1; }
struct POD { int a, b; };
struct NonTriv { NonTriv() {} };
struct Agg { int x; POD p; };

ATTR int a = 42;
ATTR int b = f();
ATTR POD c = { 1, 2 };
ATTR POD d = { 1, f() };
ATTR Agg e = { 1, { 2, 3 } };
ATTR Agg g = { 1, { f(), 3 } };
ATTR NonTriv h;
ATTR int *p = &a;
ATTR const int &r = a;
ATTR const int &r2 = f();
ATTR POD i;
ATTR POD j = POD();
