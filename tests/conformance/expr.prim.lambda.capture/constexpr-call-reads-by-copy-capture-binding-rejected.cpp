// [expr.prim.lambda.capture]/11, [basic.def.odr]/5, [expr.const]/10: a
// `constexpr` structured binding captured by copy whose address is taken
// in a constant expression in the lambda body (`*&a`) names the closure's
// member and is rejected, as the text and GCC 16.1 do (clang 22 accepts),
// also in a generic lambda, dependent or not; a read of it is accepted
// (constexpr-call-reads-by-copy-capture-binding-run.cpp, WP-1655).
//@ requires: clang>=22
//@ xfail(clang): clang 22 evaluates the bound object
//@ expect: compile-fail
//@ args: -std=c++26
//@ diag: 25:33 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 25:37 note "kiln: read of variable 'a' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 24:19 note "declared here"
//@ diag: 26:39 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 26:43 note "kiln: read of variable 'b' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 24:22 note "declared here"
//@ diag: 27:41 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 28:27 note "in instantiation of function template specialization 'f()::(lambda)::operator()<int>' requested here"
//@ diag: 27:45 note "kiln: read of variable 'a' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 24:19 note "declared here"
struct A { int x, y; };

int f() {
  constexpr auto [a, b] = A{4, 5};
  auto l1 = [=] { constexpr int j = *&a; return j; };
  auto l2 = [=](auto) { constexpr int j = *&b; return j; };
  auto l3 = [=](auto x) { constexpr int j = *&a + sizeof(x) * 0; return j; };
  return l1() + l2(0) + l3(0);
}
