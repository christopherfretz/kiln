// [temp.dep.type]/9, [temp.res.general]/4: a `decltype(e)` qualifier with
// a type-dependent operand is a dependent nested-name-specifier; names
// after it are dependent and are looked up when the qualifier is
// substituted ([temp.inst]).
//@ expect: compile-ok
struct true_type {
  static constexpr bool value = true;
  using type = true_type;
};
struct S {
  using type = int;
  static constexpr int v = 3;
  template <class T> struct M { using type = T; };
  int m;
};
S s;

// libc++ <__iterator/iterator_traits.h>: a SFINAE probe in decltype.
template <class T> struct has_type {
  template <class U> static true_type test(typename U::type * = nullptr);
  template <class U> static S test(...);
  static const bool value = decltype(test<T>(nullptr))::value;
};
struct WithType { using type = int; };
static_assert(has_type<WithType>::value);

template <class T> struct X {
  static const int value = decltype(T())::v;
  using type = typename decltype(T())::type;
  using inner = typename decltype(T())::template M<long>::type;
};
static_assert(X<S>::value == 3);
X<S>::type xi = 1;
X<S>::inner xl = 2L;

template <class T> int h(T t) { return decltype(t)::v; }
int k = h(s);

template <class T> typename decltype(T())::type ft() { return 0; }
int fi = ft<S>();

template <class T> struct Y : decltype(T())::template M<T> {};
Y<S> y;

template <class T> auto mp(T t) -> int decltype(t)::* { return &T::m; }
auto mpp = mp(s);

// A non-dependent qualifier inside a template.
template <class T> struct Z { static const int n = decltype(s)::v + sizeof(T); };
static_assert(Z<char>::n == 4);
