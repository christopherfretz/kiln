// [expr.prim.lambda.capture]/17, [temp.variadic]/5: a braced or
// parenthesized init-capture pack must expand a pack, and a non-pack
// init-capture must not leave one unexpanded.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:13 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 10:12 error "initializer contains unexpanded parameter pack 'ts'"
template <class... T> void g(T... ts) {
  auto a = [...xs{1}] {};
  auto b = [x(ts)] {};
}
