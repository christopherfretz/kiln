// A member function template of a class template partial specialization
// over a non-type pack, its trailing return type expanding its own pack
// `C...` over a pattern that names the class's substituted `I...`
// ([temp.variadic]/5, [temp.over.link]/5; spec/abi.md "Mangling",
// WP-1214): the substituted pack mangles as clang's vendor token,
// `X_SUBSTPACK_E` as a template argument and `_SUBSTPACK_` as a type
// (`tl<T, U>...` in a member of `ty<T...>`). The noexcept-specifier is
// not part of the name. A sender-library `connect` shape that kiln
// rejected with "cannot yet mangle expression type
// SubstNonTypeTemplateParmPackExpr".
template <unsigned long... I> struct idx {};
template <class T, unsigned long N> struct R {};
template <class T, class U> constexpr bool nt = true;
template <class... T> struct tup {};
template <class C, class U> struct rs {
  using type = int;
};
template <class C, class U> using res_t = typename rs<C, U>::type;
template <class... T> struct tl {};

template <class Tag, class Ix> struct conn;
template <class Tag, unsigned long... I> struct conn<Tag, idx<I...>> {
  template <class S, class... C>
  auto operator()(S &s, C &&...) const noexcept((nt<C, R<S, I>> && ...))
      -> tup<res_t<C, R<S, I>>...> {
    ++s;
    return {};
  }
};
template <class... T> struct ty {
  template <class... U> auto f(U...) const -> tup<tl<T, U>...> { return {}; }
};
int use() {
  int s = 0;
  conn<int, idx<0, 1>>{}(s, 1, 2.0);
  ty<int, char>{}.f(1, 2.0);
  return s;
}
