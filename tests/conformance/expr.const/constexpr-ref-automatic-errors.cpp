//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects
//@ diag: 36:17 error "static assertion expression is not an integral constant expression"
//@ diag: 36:17 note "read of non-const variable 'i' is not allowed in a constant expression"
//@ diag: 34:7 note "declared here"
//@ diag: 37:25 error "constexpr variable 'si' must be initialized by a constant expression"
//@ diag: 37:25 note "reference to 'i' is not a constant expression"
//@ diag: 34:7 note "declared here"
//@ diag: 39:28 error "constexpr variable 'lr' must be initialized by a constant expression"
//@ diag: 39:28 note "reference to 'q' is not a constant expression"
//@ diag: 38:15 note "address of non-static constexpr variable 'q' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 40:24 error "constexpr variable 'gp' must be initialized by a constant expression"
//@ diag: 40:24 note "pointer to 'k' is not a constant expression"
//@ diag: 32:32 note "declared here"
//@ diag: 42:24 error "constexpr variable 'lp' must be initialized by a constant expression"
//@ diag: 42:29 note "initializer of 'pp' is not a constant expression"
//@ diag: 41:18 note "declared here"
//@ diag: 43:39 error "constexpr variable 'sp' must be initialized by a constant expression"
//@ diag: 43:39 note "pointer to 'i' is not a constant expression"
//@ diag: 34:7 note "declared here"
// [expr.const.init]/2,4 (C++26, P2686R5): an automatic object is
// constexpr-referenceable only from its own function parameter scope (a
// lambda body is another one), never from a static variable's initializer
// ([expr.const.init]/4: it must be representable at namespace scope) nor
// after its function returns; a constexpr reference to a non-const local
// has a constant address but its value is not readable ([expr.const]/10).
// In a lambda body a constexpr local whose value points to an automatic
// object is not constexpr-representable, so not usable: reading it is
// "initializer of 'pp' is not a constant expression" (WP-1831).
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
