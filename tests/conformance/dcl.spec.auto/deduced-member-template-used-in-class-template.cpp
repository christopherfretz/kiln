// [dcl.spec.auto.general]/11-12, [temp.dep.expr]/3: inside a class
// template, a specialization of a member function template with a
// deduced return type may be used in a later member declaration, before
// the member template's body is parsed. The use is type-dependent (the
// enclosing class's arguments are); the return type is deduced when the
// class specialization's static member initializer is instantiated
// (Boost.Hana `detail::flatten_indices`:
// `static constexpr auto inner = compute<true>();`).
//@ expect: run
//@ stdout: 3 -3 9
extern "C" int printf(const char*, ...);

template <int N> struct S {
  template <bool B> static constexpr auto compute() { return B ? N : -N; }
  static constexpr auto inner = compute<true>();
  static constexpr auto outer = compute<false>();
  template <class T> static auto twice(T t) { return t + t; }
  using twice_t = decltype(twice(N));
  static constexpr int sq = sizeof(twice_t) * 0 + N * N;
};
static_assert(S<3>::inner == 3);
static_assert(S<3>::outer == -3);

int main() {
  S<3>::twice_t x = S<3>::sq;
  printf("%d %d %d\n", S<3>::inner, S<3>::outer, x);
}
