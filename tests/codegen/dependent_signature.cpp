// Dependent signature mangling (WP-172): typename-specifiers, dependent
// template specializations, template template parameters, dependent
// unary transforms and value-dependent template-argument expressions
// (sr unresolved names, operators, sizeof/alignof, conditionals).
namespace ns {
template <class T> struct rr { using type = T; };
template <class T> struct rr<T&> { using type = T; };
template <bool B, class T = void> struct en {};
template <class T> struct en<true, T> { using type = T; };
template <class T> struct is_i { static constexpr bool value = false; };
template <> struct is_i<int> { static constexpr bool value = true; };
template <class T> typename rr<T>::type&& mv(T&& t) { return static_cast<typename rr<T>::type&&>(t); }
template <class T> T&& fw(typename rr<T>::type& t) { return static_cast<T&&>(t); }
template <class T> struct al {
  al() {}
  template <class U> al(const al<U>&) {}
};
template <class T> typename en<is_i<T>::value, int>::type sf(T) { return 1; }
template <class T> typename en<!is_i<T>::value, long>::type sf(T) { return 2; }
template <class T> typename T::type nt(T) { return {}; }
template <class T> typename T::template rebind<int>::other rb(T) { return {}; }
}
template <class T> struct G { using type = T; };
template <class T> typename G<T>::type gt(T t) { return t; }
template <class T> int gs(G<T>) { return 0; }
struct H { using type = int; template <class U> struct rebind { using other = U; }; };
int use() {
  int x = 0;
  int y = ns::mv(x);
  int& z = ns::fw<int&>(x);
  ns::al<int> a; ns::al<long> b(a);
  return y + z + ns::sf(1) + (int)ns::sf(1.0) + ns::nt(H{}) + ns::rb(H{}) + gt(3) + gs(G<int>{});
}
template <class T> __remove_reference_t(T)&& umv(T&& t) { return static_cast<__remove_reference_t(T)&&>(t); }
template <class T> T&& ufw(__remove_reference_t(T)& t) { return static_cast<T&&>(t); }
template <class T> __decay(T) udc(T t) { return t; }
template <int N> struct I { static constexpr int value = N; using type = int; };
template <class T> typename I<sizeof(T)>::type szt(T) { return 0; }
template <class T> typename I<sizeof(T) * 2 + 1>::type szx(T) { return 0; }
template <int N> typename I<N>::type nttp() { return N; }
template <int N> typename I<N - 1>::type nttp2() { return N; }
template <class T> typename I<(T::value > 0) ? 1 : 2>::type cnd(T) { return 0; }
template <class T> typename I<alignof(T)>::type alt(T) { return 0; }
template <template <class> class TT, class T> typename TT<T>::type ttp(T) { return 0; }
template <class T> typename T::template rebind<long>::other dts(T) { return 0; }
template <class T> typename ns::en<T::value == 3 || false, int>::type ex2(T) { return 0; }
template <class T> typename T::type::type nn2(T) { return 0; }
struct J { using type = G<int>; };
int use2() {
  int i = 0;
  int& r = ufw<int&>(i);
  return umv(i) + r + udc(i) + szt(i) + szx('c') + nttp<3>() + nttp2<4>() + cnd(I<5>{})
       + alt(1.0) + ttp<G>(1) + (int)dts(H{}) + ex2(I<3>{}) + nn2(J{});
}
