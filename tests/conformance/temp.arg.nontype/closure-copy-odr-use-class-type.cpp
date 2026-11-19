// [temp.arg.nontype]/2, [expr.prim.lambda.capture]/11, [expr.const]/10: a
// template argument for a non-type parameter of class type inside a lambda
// body that odr-uses a by-copy capture (`I<v[K]>`, `operator[]` returning
// a reference into a `constexpr` local of class type) reads the captured
// variable, as a scalar argument does (closure-copy-odr-use-reads-variable-run.cpp,
// a recorded deviation from the text that clang 22 and GCC 16.1 share), in
// a lambda, a generic lambda and a lambda with a template parameter pack
// expanded in the argument, also under a consteval function template.
// Reduced from real code (a sender library's completion signatures).
//@ expect: compile-ok
//@ std: c++20
struct TI { int v; };
template <TI N> struct I { static constexpr int v = N.v; };
struct V {
  TI a[3];
  constexpr const TI& operator[](int i) const { return a[i]; }
  constexpr int get(int i) const { return a[i].v; }
};
template <int... Is> struct Seq {};
template <class... Ts> struct List { static constexpr int n = (Ts::v + ... + 0); };

void g() {
  constexpr V v{{{1}, {2}, {3}}};
  auto a = [=] { return I<v[1]>{}; };
  auto b = [=](auto) { return I<v[2]>{}; };
  auto c = [=](auto) { return I<TI{v.get(0)}>{}; };
  auto d = [=]<int K>(Seq<K>) { return I<v[K]>{}; };
  static_assert(decltype(a())::v == 2);
  static_assert(decltype(b(0))::v == 3);
  static_assert(decltype(c(0))::v == 1);
  static_assert(decltype(d(Seq<0>{}))::v == 1);
}

template <class T> consteval auto sum() {
  constexpr V v{{{T::a}, {T::b}, {T::c}}};
  constexpr auto fn = [=]<int... Is>(Seq<Is...>) { return List<I<v[Is]>...>(); };
  return fn(Seq<0, 1, 2>{});
}
struct Abc { static constexpr int a = 1, b = 2, c = 4; };
static_assert(decltype(sum<Abc>())::n == 7);
