// [expr.prim.lambda.capture]/11, [expr.const]/10, [stmt.switch]/2: a case
// value is a converted constant expression and a full-expression
// ([intro.execution]/5); an odr-use in it of a constant captured by copy
// names the closure's member, which no constant expression evaluated in
// the lambda body outside a call of the call operator can read. The
// full-expression ends (deciding which names are odr-uses) before the
// value is evaluated, so `case *&i:` is rejected like `constexpr int j =
// *&i;`: explicit `[i]`, `[=]`, a GNU case range, a generic lambda and a
// lambda in a template. GCC 16.1 rejects these; clang 19 and 22 read the
// variable and accept: the text decides. An inner `[&]` of an outer `[=]`
// (/12) is accepted by both and reads the variable in kiln too (WP-1655,
// constexpr-call-reads-by-copy-capture-run.cpp). Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ requires: clang>=22
//@ xfail(clang): clang reads the enclosing variable
//@ diag: 39:44 error "case value is not a constant expression"
//@ diag: 39:44 note "kiln: read of variable 'i' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 38:17 note "declared here"
//@ diag: 40:44 error "case value is not a constant expression"
//@ diag: 40:44 note "kiln: read of variable 'i' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 38:17 note "declared here"
//@ diag: 41:50 error "case value is not a constant expression"
//@ diag: 41:50 note "kiln: read of variable 'i' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 38:17 note "declared here"
//@ diag: 42:45 error "case value is not a constant expression"
//@ diag: 43:41 note "in instantiation of function template specialization 'f()::(lambda)::operator()<int>' requested here"
//@ diag: 42:45 note "kiln: read of variable 'i' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 38:17 note "declared here"
//@ diag: 47:43 error "case value is not a constant expression"
//@ diag: 47:23 note "while substituting into a lambda expression here"
//@ diag: 50:15 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 47:43 note "kiln: read of variable 'i' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 46:15 note "declared here"
void f() {
  constexpr int i = 1;
  auto l1 = [i](int k) { switch (k) { case *&i: return 1; } return 0; };
  auto l2 = [=](int k) { switch (k) { case *&i: return 1; } return 0; };
  auto l3 = [=](int k) { switch (k) { case 0 ... *&i: return 1; } return 0; };
  auto l5 = [=](auto k) { switch (k) { case *&i: return 1; } return 0; };
  (void)l1; (void)l2; (void)l3; (void)l5(0);
}
template <class T> void g() {
  constexpr T i = 1;
  auto l = [=](int k) { switch (k) { case *&i: return 1; } return 0; };
  (void)l;
}
template void g<int>();
