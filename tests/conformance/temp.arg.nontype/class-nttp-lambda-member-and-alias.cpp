// [temp.arg.nontype]/1, [temp.type]/2: a structural class holding a
// lambda as a class NTTP argument (two uses of the same object are the
// same specialization, distinct closure types are not), and a value-
// dependent int argument for a class-type parameter of an alias or
// class template, converted before the alias is substituted (WP-1497).
//@ expect: compile-ok
//@ std: c++20
template <class F> struct W { F f; };
template <auto V> struct T {};
constexpr auto l = [] { return 1; };
static_assert(__is_same(T<W{l}>, T<W{l}>));
using L1 = decltype([] {});
static_assert(!__is_same(T<W<L1>{}>, T<W<decltype([] {})>{}>));
template <class U> struct M { template <auto V> static constexpr bool same = __is_same(T<V>, T<W{l}>); };
static_assert(M<int>::same<W{l}>);
struct n { constexpr n(int a) : i(a) {} int i; };
template <n N> using i_type = decltype(N.i);
template <int X> using j_type = i_type<X>;
static_assert(__is_same(j_type<3>, int));
template <n N> struct S { static constexpr int v = N.i; };
template <int X> using s_type = S<X>;
static_assert(__is_same(s_type<3>, S<n{3}>));
static_assert(s_type<4>::v == 4);
