// [temp.names], [dcl.type.class.deduct]/2, [temp.arg.template]: a
// template template parameter named alone is a placeholder for a deduced
// class type, deduced once its argument is known (libc++ `ranges::to`'s
// `decltype(_Container(std::declval<_Range>()))`).
//@ std: c++26
template <class T> struct A {
  constexpr A(T t) : t(t) {}
  T t;
};
template <class T> struct P {
  T a, b;
};

template <template <class...> class C, class R> constexpr int cast() { return C(R(3)).t; }
static_assert(cast<A, int>() == 3);

template <template <class...> class C, class R> constexpr int var() {
  C c(R(4));
  return c.t;
}
static_assert(var<A, int>() == 4);

template <template <class...> class C, class R> constexpr int list() { return C{R(5)}.t; }
static_assert(list<A, int>() == 5);

template <template <class...> class C, class R> using D = decltype(C(R()));
static_assert(__is_same(D<A, long>, A<long>));

template <template <class> class C> constexpr int aggr() { return C{1, 2}.b; }
static_assert(aggr<P>() == 2);

template <template <class...> class C, class R> constexpr bool can = requires { C(R()); };
static_assert(can<A, int>);
