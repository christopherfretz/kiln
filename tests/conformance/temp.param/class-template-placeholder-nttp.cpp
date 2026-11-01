// [temp.param]/6.3, [temp.arg.nontype]/1, [dcl.type.class.deduct]/1: a
// non-type template parameter declared with a class template placeholder
// (`template<std::array A>`) deduces its type from each argument as for
// `C x = e;`; a use of the parameter is type-dependent, and
// `decltype(A)` names the deduced class type ([dcl.type.decltype]/1.3).
// The libc++ `array.overview/nttp.equivalence` test.
//@ expect: compile-ok
//@ strict-diags
template <class T, unsigned N> struct arr { T e[N]; };
template <class T, class... U> arr(T, U...) -> arr<T, 1 + sizeof...(U)>;

template <class T, class U> struct same { static const bool value = false; };
template <class T> struct same<T, T> { static const bool value = true; };

template <arr A> struct test { static const bool value = false; };
template <> struct test<arr<int, 3>{4, 5, 6}> { static const bool value = true; };
static_assert(!test<arr<int*, 4>{}>::value);
static_assert(!test<arr<int, 3>{}>::value);
static_assert(!test<arr<int, 3>{4, 5}>::value);
static_assert(!test<arr<long, 3>{4, 5, 6}>::value);
static_assert(test<arr<int, 3>{4, 5, 6}>::value);

template <arr A> struct uses {
  static constexpr auto first = A.e[0];
  using type = decltype(A);
  static_assert(same<decltype((A)), const type&>::value);
};
static_assert(uses<arr{1L, 2L}>::first == 1);
static_assert(same<uses<arr{1L, 2L}>::type, arr<long, 2>>::value);

template <class T> struct W { T t; };
template <W w> constexpr auto get() { return w.t; }
static_assert(get<W{3}>() == 3);
static_assert(same<decltype(get<W{'c'}>()), char>::value);
