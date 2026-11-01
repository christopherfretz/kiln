//@ expect: compile-ok
//@ strict-diags
// [temp.spec.partial.order], [temp.deduct.type]/9: when ordering partial
// specializations, a pack deduced from no elements is empty, so `C<A>` is
// more specialized than `C<A, B...>` (libc++ `conjunction<_Arg>` vs
// `conjunction<_Arg, _Args...>`).
template <class... T> struct C { static constexpr int v = 0; };
template <class A> struct C<A> { static constexpr int v = 1; };
template <class A, class... B> struct C<A, B...> { static constexpr int v = 2; };
static_assert(C<>::v == 0);
static_assert(C<int>::v == 1);
static_assert(C<int, int>::v == 2);

template <class... T> struct D { static constexpr int v = 0; };
template <class A, class... B> struct D<A, B...> { static constexpr int v = 1; };
template <class A, class... B> struct D<A*, B...> { static constexpr int v = 2; };
template <class A> struct D<A*> { static constexpr int v = 3; };
static_assert(D<int>::v == 1);
static_assert(D<int*>::v == 3);
static_assert(D<int*, int>::v == 2);

template <int... N> struct E { static constexpr int v = 0; };
template <int M> struct E<M> { static constexpr int v = 1; };
template <int M, int... N> struct E<M, N...> { static constexpr int v = 2; };
static_assert(E<1>::v == 1);
static_assert(E<1, 2>::v == 2);
