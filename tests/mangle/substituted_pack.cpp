// Mangling differential: a pack substituted into a member template's
// signature that still names the member template's own unexpanded pack
// ([temp.variadic]/5, [temp.over.link]/5; spec/abi.md "Mangling",
// WP-1214). Once `conn<Tag, idx<0, 1>>` is instantiated, `I...` is
// substituted while `C...` is still dependent, so the pattern `R<S, I>`
// of `C...`'s expansion holds a substituted non-type pack and `tl<T, U>`
// a substituted type pack. The Itanium ABI has no production; clang 19
// and 22 write the vendor token `_SUBSTPACK_`: `X_SUBSTPACK_E` as a
// template argument (not a substitution candidate), a bare
// `_SUBSTPACK_` as a type (a candidate). A noexcept-specifier is not
// part of the name, so the noexcept-only form is the plain signature.
template <unsigned long... I> struct idx {};
template <class T, unsigned long N> struct R {};
template <class T, class U> constexpr bool nt = true;
template <class... T> struct tup {};
template <class C, class U> struct rs { using type = int; };
template <class C, class U> using res_t = typename rs<C, U>::type;
template <class... T> struct tl {};

template <class Tag, class Ix> struct conn;
template <class Tag, unsigned long... I>
struct conn<Tag, idx<I...>> {
  template <class S, class... C>
  auto operator()(S &, C &&...) const noexcept((nt<C, R<S, I>> && ...))
      -> tup<res_t<C, R<S, I>>...> {
    return {};
  }
};
template <class Ix> struct ne;
template <unsigned long... I> struct ne<idx<I...>> {
  template <class S, class... C>
  void f(S &, C &&...) const noexcept((nt<C, R<S, I>> && ...)) {}
};
template <class Ix> struct tr;
template <unsigned long... I> struct tr<idx<I...>> {
  template <class S, class... C>
  auto f(S &, C &&...) const -> tup<res_t<C, R<S, I>>...> {
    return {};
  }
};
template <class Ix> struct ns;
template <unsigned long... I> struct ns<idx<I...>> {
  template <class... C> struct in {
    template <class S> auto f(S &) const -> tup<R<C, I>...> { return {}; }
  };
  template <class S, class... C>
  auto g(S &, C...) const -> tup<tl<R<C, I>, R<S, I>>...> {
    return {};
  }
};
template <class... T> struct ty {
  template <class... U> auto f(U...) const -> tup<tl<T, U>...> { return {}; }
  template <class... U> auto h(U...) const -> tup<tl<T, U, T>...> { return {}; }
};
void use() {
  int s = 0;
  conn<int, idx<0, 1>>{}(s, 1, 2.0);
  ne<idx<0, 1>>{}.f(s, 1, 2.0);
  tr<idx<0, 1>>{}.f(s, 1, 2.0);
  ns<idx<0, 1>>::in<int, char>{}.f(s);
  ns<idx<0, 1>>{}.g(s, 1, 2.0);
  ty<int, char>{}.f(1, 2.0);
  ty<int, char>{}.h(1, 2.0);
}
