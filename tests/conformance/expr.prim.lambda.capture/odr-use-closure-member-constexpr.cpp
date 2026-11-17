// [expr.prim.lambda.capture]/11: an id-expression in a lambda body that is
// an odr-use of an entity captured by copy names the closure's member, and
// [expr.const]/10 makes such an odr-use non-constant in an expression
// evaluated in the body outside a call of the call operator (the example
// there: `constexpr int j = *&n;`). Reading the member, directly or through
// a reference or `this`, is diagnosed; its address is not constant (as
// for the variable, clang's note). Siblings: explicit `[x]`, `[=]`, a
// capture of a capture (an inner `[&]` of an outer `[=]` names the outer
// closure's member, /12), a generic lambda, a structured binding (always
// odr-used when potentially evaluated, [basic.def.odr]/5), a class object
// whose member function reads it, `static_assert`. clang 19 and 22 read
// the variable and accept all but the address; GCC 16.1 rejects `*&` and
// the address but accepts the read through a reference parameter or
// `this`, the capture of a capture and the binding: the text decides.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ xfail(clang): clang reads the enclosing variable
//@ diag: 60:33 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 60:37 note "kiln: read of variable 'n' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 56:13 note "declared here"
//@ diag: 61:33 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 61:37 note "kiln: read of variable 'x' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 57:17 note "declared here"
//@ diag: 62:33 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 54:41 note "kiln: read of variable 'x' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 62:37 note "in call to 'rd(x)'"
//@ diag: 57:17 note "declared here"
//@ diag: 63:33 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 52:38 note "kiln: read of variable 's' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 63:37 note "in call to 's.val()'"
//@ diag: 58:15 note "declared here"
//@ diag: 64:46 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 64:50 note "kiln: read of variable 'n' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 56:13 note "declared here"
//@ diag: 65:39 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 65:43 note "kiln: read of variable 'x' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 57:17 note "declared here"
//@ diag: 66:33 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 66:37 note "kiln: read of variable 'a' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 59:19 note "declared here"
//@ diag: 67:33 error "static assertion expression is not an integral constant expression"
//@ diag: 67:33 note "kiln: read of variable 'n' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 56:13 note "declared here"
//@ diag: 68:40 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 68:40 note "pointer to 'n' is not a constant expression"
//@ diag: 56:13 note "declared here"
struct P { int x, y; };
struct S {
  int v;
  constexpr int val() const { return v; }
};
constexpr int rd(const int &r) { return r; }
void g() {
  const int n = 1;
  constexpr int x = 2;
  constexpr S s{3};
  constexpr auto [a, b] = P{4, 5};
  auto l1 = [=] { constexpr int j = *&n; return j; };
  auto l2 = [x] { constexpr int j = *&x; return j; };
  auto l3 = [=] { constexpr int j = rd(x); return j; };
  auto l4 = [=] { constexpr int j = s.val(); return j; };
  auto l5 = [=] { return [&] { constexpr int j = *&n; return j; }(); };
  auto l6 = [=](auto) { constexpr int j = *&x; return j; };
  auto l7 = [=] { constexpr int j = a; return j; };
  auto l8 = [=] { static_assert(*&n == 1); };
  auto l9 = [=] { constexpr const int *p = &n; return p; };
  (void)l1; (void)l2; (void)l3; (void)l4; (void)l5; (void)l6(0); (void)l7; (void)l8; (void)l9;
}
