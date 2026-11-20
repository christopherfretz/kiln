//@ args: -Wall -Wextra
//@ strict-diags
// [temp.dep.constexpr]: a value-dependent expression has no value in the
// template definition. Warnings that look at constant conditions in a
// template definition leave them alone (clang evaluates none); the
// instantiations fold as usual.
template <int... N> constexpr int packs() {
  if (sizeof...(N) > 2)
    return (N + ... + 0);
  if ((N + ... + 0) == 0)
    return -1;
  while ((N && ... && false)) {
  }
  return (N * ... * 1);
}
static_assert(packs<1, 2, 3>() == 6);
static_assert(packs<>() == -1);
static_assert(packs<2, 3>() == 6);

template <class T> struct S {
  static constexpr int v = sizeof(T);
  int f() {
    if (v > 4 || T::value)
      return v << T::shift;
    return sizeof(T) / T::div;
  }
};
struct A {
  static constexpr bool value = true;
  static constexpr int shift = 1;
  static constexpr int div = 2;
};
int g() { return S<A>().f(); }
