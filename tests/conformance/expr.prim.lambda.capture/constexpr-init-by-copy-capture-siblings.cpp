// [expr.prim.lambda.capture]/8, /11, /12, [basic.def.odr]/5, [expr.const]/10:
// a constant expression evaluated in a lambda body that odr-uses a by-copy
// capture names the closure's member, which needs the closure object: no
// constant expression. Not odr-used (the lvalue-to-rvalue conversion of a
// constant, a member read of a constant object), the name denotes the
// enclosing variable and its value is constant, also when it is captured
// explicitly. Odr-used: its address (`*&n`, `&n`), a reference binding, a
// nested lambda, a generic lambda, an init-capture, `this`. A by-reference
// capture names the variable itself. kiln agrees with GCC 16.1 on every
// case; clang 22 accepts the address forms, the nested and the generic
// lambda (WP-1535).
//@ xfail(clang): clang 19 and 22 evaluate a captured constant's own value
//@ expect: compile-fail
//@ args: -Wno-unused-value -Wno-unused-lambda-capture
//@ diag: 25:50 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 26:57 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 27:57 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 29:54 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 30:67 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 31:65 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 33:60 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 35:51 error "constexpr variable 'j' must be initialized by a constant expression"
struct P { int a; };
void f1() { const int n = 1; [=] { constexpr int i = n; return i; }; }
void f2() { const int n = 1; [=] { constexpr int j = *&n; return j; }; }
void f3() { const int n = 1; [=] { constexpr const int* p = &n; return p; }; }
void f4() { const int n = 1; [=] { constexpr const int& r = n; return r; }; }
void f5() { const int n = 1; [&] { constexpr int j = *&n; return j; }; }
void f6() { const int n = 1; [m = n] { constexpr int j = m; return j; }; }
void f7() { constexpr int n = 1; [=] { return [=] { constexpr int j = *&n; return j; }(); }; }
void f8() { const int n = 1; auto l = [=](auto) { constexpr int j = *&n; return j; }; l(0); }
void f9() { constexpr P p{1}; [=] { constexpr int j = p.a; return j; }; }
struct S { int m = 1; void g() { [=, this] { constexpr int j = m; return j; }; } };
void f11() { const int n = 1; [n] { constexpr int j = n; return j; }; }
void f12() { const int n = 1; [n] { constexpr int j = *&n; return j; }; }
