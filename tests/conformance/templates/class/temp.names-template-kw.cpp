// [temp.names]/5, [temp.res.general]/4: 'T::template X<args>' names a
// member template of a dependent type; it is resolved at instantiation.
template <class From> struct copy_cv { template <class To> using apply = const To; };
template <class T, class U> struct S { using type = typename T::template apply<U>; };
S<copy_cv<int>, long>::type v = 1;
template <class T> struct H { template <class U> struct In { using type = U; }; };
template <class T> using G = typename H<T>::template In<T>::type;
G<int> g = 2;
struct A {
  template <class U> static constexpr int n = sizeof(U);
  template <class U> struct Inner { static constexpr int v = sizeof(U) * 2; };
};
template <class T> struct B {
  static constexpr int m = T::template n<long>;
  static constexpr int k = T::template Inner<short>::v;
  using I = typename T::template Inner<char>;
};
static_assert(B<A>::m == 8);
static_assert(B<A>::k == 4);
static_assert(B<A>::I::v == 2);
