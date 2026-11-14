// Specializations a precompiled header's snapshot carries (WP-1344).
#pragma once
namespace r1 {
template <class...> struct L {};
template <class T, class... U> struct L<T, U...> : L<U...> { static constexpr int n = sizeof(T); };
template <int N, class T, bool = (N <= T::n)> struct sel;
template <int N, class U, class... Us> struct sel<N, L<U, Us...>, true> { using type = U; };
template <int N, class U, class... Us> struct sel<N, L<U, Us...>, false> : sel<N, L<Us...>> {};
template <class T> struct pick {
  using Ls = L<unsigned char, unsigned short, unsigned int, unsigned long>;
  using type = typename sel<sizeof(T), Ls>::type;
};
}
namespace r6 {
template <class A, class B, int N = 1, class = void>
struct impl : impl<A, B, N + 1> {};
template <class A, class B> struct impl<A, B, 3, void> { using type = int; };
template <class A, class B> struct impl<A, B, 2, decltype(void(sizeof(A) == 4))> { using type = A; };
template <class T> struct chain : chain<T*> {};
template <class T> struct chain<T***> { static constexpr int depth = 3; };
}
namespace r7 {
typedef unsigned long sz;
template <sz... I> struct smax;
template <sz I0> struct smax<I0> { static const sz value = I0; };
template <sz I0, sz I1, sz... In> struct smax<I0, I1, In...> {
  static const sz value = I0 >= I1 ? smax<I0, In...>::value : smax<I1, In...>::value;
};
template <sz L, class T0, class... Ts> struct au {
  static const sz len = smax<L, sizeof(T0), sizeof(Ts)...>::value;
};
template <int... N> struct seq {};
template <class S, int K> struct add;
template <int... N, int K> struct add<seq<N...>, K> { using type = seq<N..., K>; };
}
namespace r9 {
typedef unsigned long sz;
template <class T, T... V> struct iseq { static constexpr sz n = sizeof...(V); };
template <sz... I> struct idx {};
template <class T, T N> using make_seq = __make_integer_seq<iseq, T, N>;
template <sz N> struct mk {
  template <class, sz... I> using tup = idx<I...>;
  using type = __make_integer_seq<tup, sz, N>;
};
template <sz I, class... T> struct nth { using type = __type_pack_element<I, T...>; };
}
namespace r4 {
template <class T> struct A { static constexpr int n = sizeof(T); };
template <class T, bool = (A<T>::n > 2)> struct B { static constexpr int v = 1; };
template <class T> struct B<T, false> { static constexpr int v = 2; };
template <class T> struct C { using t = B<T>; int x; };
inline int c = sizeof(C<int>);
inline int d = sizeof(C<char>);
}
