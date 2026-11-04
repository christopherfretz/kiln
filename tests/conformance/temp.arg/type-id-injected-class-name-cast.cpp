// [temp.arg.general]/2, [dcl.ambig.res]/3: the type-id reading of
// `T(name)` wins only when `name` is a type. `int(x)` with `x` a variable
// is a functional cast (no type-id declares a name), so it is a non-type
// template argument, also inside a class template next to its
// injected-class-name; for a type parameter it is rejected.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:21 error "template argument for template type parameter must be a type"
//@ diag: 11:17 note "template parameter is declared here"
template <int N> struct I { static constexpr int value = N; };
template <class T> struct Tag { static constexpr int value = -1; };
constexpr long x = 7;

template <class... Ts> struct CT {
  static constexpr long y = 3;
  static_assert(I<int(x)>::value == 7);
  static_assert(I<int(y)>::value == 3);
  static_assert(I<int(x) + 1>::value == 8);
  static_assert(Tag<void(CT)>::value == -1);
  static_assert(Tag<int(y)>::value == -1);
};
CT<int> a;
