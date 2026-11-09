// [expr.add]/4, [dcl.ref]/5: clang 19 does not check a reference data
// member (or a reference variable) as it binds, only a constant
// expression's final value (`CheckLValueConstantExpression`), so only the
// namespace-scope variable fails here. The clang 22 behaviour is
// ref-member-past-end-clang22.cpp. WP-1021.
//@ std: c++26
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:41 warning "left operand of comma operator has no effect [-Wunused-value]"
//@ diag: 34:13 error "constexpr variable 'g' must be initialized by a constant expression"
//@ diag: 34:13 note "dereferenced pointer past the end of subobject of 'a' is not a constant expression"
//@ diag: 14:15 note "declared here"
constexpr int a[2] = {1, 2};
constexpr const int *end = a + 2;
struct R { const int &r; };
struct D { const int &r = *end; };
struct C { const int &r; constexpr C(const int *p) : r(p[2]) {} };
struct B : R {};
constexpr int f1(const int *p) { R x{p[2]}; return 0; }
constexpr int k1 = f1(a);
constexpr int f2(const int *p) { R x{.r = p[2]}; return 0; }
constexpr int k2 = f2(a);
constexpr int f3(const int *p) { R *q = new R{p[2]}; delete q; return 0; }
constexpr int k3 = f3(a);
constexpr int f4(const int *p) { return R{p[2]}, 0; }
constexpr int k4 = f4(a);
constexpr int f5() { D d; return 0; }
constexpr int k5 = f5();
constexpr int f6() { C c(a); return 0; }
constexpr int k6 = f6();
constexpr int f7(const int *p) { B b{{p[2]}}; return 0; }
constexpr int k7 = f7(a);
constexpr R g{*end};
