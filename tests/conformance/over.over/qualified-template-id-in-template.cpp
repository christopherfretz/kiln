// [over.over]/3 with [temp.names]/5 (WP-297): a qualified member function
// template-id in a dependent scope (`Disp<I>::template dispatch<F, V>`) that
// is not called names its one specialization once substituted, as an
// unqualified one does, so a deduced `auto` return type is the function
// pointer type (libc++ variant visitation `__make_dispatch`; every
// std::variant relational operator failed with "address of overloaded
// function 'dispatch' does not match required type").
//@ expect: run
//@ strict-diags
template <int... Is> struct seq {};
struct Base {
  template <int... Is>
  struct Disp {
    template <class F, class... Vs>
    static constexpr decltype(auto) dispatch(F f, Vs... vs) { return f(vs..., Is...); }
  };
  template <class F, class... Vs, int... Is>
  static constexpr auto make(seq<Is...>) {
    return Disp<Is...>::template dispatch<F, Vs...>;
  }
};
template <class T> struct S {
  template <class U> static int conv(U u) { return int(u) + int(sizeof(T)); }
};
template <class T> int called(T t) { return S<T>::template conv<T>(t); }
int add(int a, int b) { return a + b; }
int main() {
  constexpr auto p = Base::make<int (*)(int, int), int>(seq<2>{});
  static_assert(__is_same(decltype(p), int (*const)(int (*)(int, int), int)));
  return p(add, 1) == 3 && called<char>('\1') == 2 ? 0 : 1;
}
