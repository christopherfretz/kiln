// [dcl.type.cv]/4, [expr.const]/5.8: writing a const local through a
// glvalue that casts its const away (`const_cast<int&>(x) = 2`, `++`,
// a C-style `(int&)x`) is not a constant expression, as through a
// reference or a pointer; reading through the cast is fine. clang is the
// oracle.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:59 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 21:15 note "in call to 'a()'"
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:38 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 23:15 note "in call to 'b()'"
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:47 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 25:15 note "in call to 'c()'"
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:71 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 27:15 note "in call to 'd()'"
constexpr int a() { const int x = 1; const_cast<int &>(x) = 2; return x; }
static_assert(a());
constexpr int b() { const int x = 1; ++const_cast<int &>(x); return x; }
static_assert(b());
constexpr int c() { const int x = 1; (int &)x = 5; return x; }
static_assert(c());
constexpr int d() { const int x = 1; int &r = const_cast<int &>(x); r = 2; return x; }
static_assert(d());
// Reading through the cast is fine.
constexpr int e() { const int x = 4; return const_cast<int &>(x) + 1; }
static_assert(e() == 5);
