// [temp.variadic]/5, [expr.prim.lambda.general]/1: the requires-clause of
// a generic lambda's template-head may name a pack of an enclosing
// template; a pack expansion around the lambda-expression instantiates one
// closure per element, each constrained by its element. clang 19 accepts;
// clang 22 rejects the constant evaluation (and crashes in codegen) when
// the clause names an outer pack, and GCC 16.1 ICEs: oracle defects.
// WP-776.
//@ expect: compile-ok
//@ requires: clang<22
template <class T, class U> concept Same = __is_same(T, U);
template <class... Ts> constexpr int head() {
  return (0 + ... + []<class U> requires Same<U, Ts> (U) { return 1; }(Ts{}));
}
template <class... Ts> constexpr int both() {
  return (0 + ... + []<class U> requires Same<U, Ts> (U) requires (sizeof(U) == sizeof(Ts)) {
    return 1;
  }(Ts{}));
}
static_assert(head<int, long>() == 2);
static_assert(both<int, long, char>() == 3);
