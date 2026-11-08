// [expr.ref]/7.5, [expr.const]/10: in `E1->E2` naming an enumerator the
// object expression is still evaluated, so a pointer parameter whose value
// is unknown makes it non-constant (clang). (`ref.E` with `ref` a
// reference parameter is accepted from clang 20, P2280.) WP-811.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:27 error "case value is not a constant expression"
//@ diag: 13:27 note "function parameter 'q' with unknown value cannot be used in a constant expression"
//@ diag: 11:22 note "declared here"
struct R { enum T { A, B = 5 }; T type; };
int f(R response, R *q) {
  int n = 0;
  switch (q->type) { case q->B: n += 10; break; default: break; }
  return n + response.A;
}
