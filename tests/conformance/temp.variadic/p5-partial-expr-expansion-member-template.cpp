// [temp.variadic]/5, [temp.inst]/3: instantiating a class template
// substitutes the class's pack into a member template's declaration while
// the member's own pack stays dependent. An expression pack expansion in
// a template argument list naming both packs (`B<P<Ts, Us>::value...>`)
// stays an expansion, partially substituted, and expands when the member
// is used (the reduced range-v3 `common_tuple` `args_` shape).
//@ expect: run
//@ std: c++20
//@ strict-diags
template <bool... Bs> struct all { static constexpr bool value = (Bs && ...); };
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
template <bool B, class T> struct enable {};
template <class T> struct enable<true, T> { using type = T; };
template <class... Ts> struct L {};

template <int N, class... Ts> struct A {
  template <class... Us>
  typename enable<all<same<Ts, Us>::value...>::value, int>::type f(Us...) { return N; }
  template <class... Us> all<sizeof(L<Ts, Us>) == 1 ...> g(Us...) { return {}; }
};

int main() {
  A<7, int, long> a;
  static_assert(decltype(a.g(1, 2))::value);
  return a.f(1, 2L) == 7 ? 0 : 1;
}
