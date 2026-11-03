//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects
//@ diag: 33:17 error "static assertion expression is not an integral constant expression"
//@ diag: 33:17 note "read of non-const variable 'i' is not allowed in a constant expression"
//@ diag: 31:7 note "declared here"
//@ diag: 34:25 error "constexpr variable 'si' must be initialized by a constant expression"
//@ diag: 34:25 note "reference to 'i' is not a constant expression"
//@ diag: 31:7 note "declared here"
//@ diag: 36:28 error "constexpr variable 'lr' must be initialized by a constant expression"
//@ diag: 36:28 note "reference to 'q' is not a constant expression"
//@ diag: 35:15 note "declared here"
//@ diag: 37:24 error "constexpr variable 'gp' must be initialized by a constant expression"
//@ diag: 37:24 note "pointer to 'k' is not a constant expression"
//@ diag: 29:32 note "declared here"
//@ diag: 39:24 error "constexpr variable 'lp' must be initialized by a constant expression"
//@ diag: 39:24 note "pointer to 'parm' is not a constant expression"
//@ diag: 30:12 note "declared here"
//@ diag: 40:39 error "constexpr variable 'sp' must be initialized by a constant expression"
//@ diag: 40:39 note "pointer to 'i' is not a constant expression"
//@ diag: 31:7 note "declared here"
// [expr.const.init]/2,4 (C++26, P2686R5): an automatic object is
// constexpr-referenceable only from its own function parameter scope (a
// lambda body is another one), never from a static variable's initializer
// ([expr.const.init]/4: it must be representable at namespace scope) nor
// after its function returns; a constexpr reference to a non-const local
// has a constant address but its value is not readable ([expr.const]/10).
struct Q { int a, b; };
constexpr const int *g() { int k = 1; const int *p = &k; return p; }
void f(int parm) {
  int i = 3;
  constexpr int &ri = i;
  static_assert(ri == 3);
  static constexpr int &si = i;
  constexpr Q q{1, 2};
  [&] { constexpr const Q &lr = q; }();
  constexpr const int *gp = g();
  constexpr int *pp = &parm;
  [&] { constexpr int *lp = pp; }();
  static constexpr struct { int *p; } sp{&i};
}
