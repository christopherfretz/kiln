// [temp.variadic]/5, [expr.prim.lambda.general]/1: a lambda's trailing
// requires-clause and the default arguments of its template parameters may
// name a pack of an enclosing template; the lambda-expression then contains
// that pack and a pack expansion around it instantiates one closure per
// element, whose constraint is checked with that element (clang 19 and 22
// accept; GCC 16.1 does not see the pack and rejects the folds). The
// template-head's requires-clause:
// p5-lambda-template-head-requires-outer-pack.cpp. WP-776.
//@ expect: compile-ok
template <class T, class U> concept Same = __is_same(T, U);
template <class T> concept Int = __is_same(T, int);
template <class T> struct Box { using type = T; };

template <class... Ts> constexpr int trailing() {
  return (0 + ... + []<class U>(U) requires Same<U, Ts> { return 1; }(Ts{}));
}
// The pack is named only in the constraint.
template <class... Ts> constexpr int only_in_constraint() {
  return (0 + ... + []<class U>(U) requires Same<U, Ts> { return 1; }(0));
}
template <class... Ts> constexpr int generic() {
  return (0 + ... + [](auto u) requires Same<decltype(u), Ts> { return 1; }(0));
}
template <class... Ts> constexpr int with_default() {
  return (0 + ... + []<class U = Ts>() requires Int<typename Box<U>::type> { return 1; }());
}
template <class... Ts> constexpr int default_sizes() {
  return (0 + ... + []<class U = Ts>() { return (int)sizeof(U); }());
}
template <int... Ns> constexpr int default_values() {
  return (0 + ... + []<int M = Ns * 2>() { return M; }());
}
constexpr int nested() {
  return [](auto... xs) {
    return (0 + ... + []<class U = decltype(xs)>() { return (int)sizeof(U); }());
  }(char(1), short(2), 3);
}
static_assert(trailing<int, long>() == 2);
static_assert(only_in_constraint<int, int>() == 2);
static_assert(generic<int, int>() == 2);
static_assert(with_default<int, int>() == 2);
static_assert(default_sizes<char, int>() == 1 + sizeof(int));
static_assert(default_values<1, 2, 3>() == 12);
static_assert(nested() == 1 + sizeof(short) + sizeof(int));
