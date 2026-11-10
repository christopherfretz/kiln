// A pack substituted into a member template's signature that still
// expands the member template's own pack ([temp.variadic]/5,
// [temp.over.link]/5; spec/abi.md "Mangling", WP-1214): the
// specializations below are declared here and defined in
// Inputs/substituted-pack-defs.cpp under the names clang 19 and 22 give
// them (asm labels), so this links only if kiln's names are clang's:
// `X_SUBSTPACK_E` for the substituted non-type pack `I` in `R<S, I>`
// (trailing return and noexcept forms, nested), `_SUBSTPACK_` for the
// substituted type pack `T` in `tl<T, U>`, and a default argument and an
// implicit constructor call in a dependent operand.
//@ expect: run
//@ args: %{test_dir}/Inputs/substituted-pack-defs.cpp
//@ stdout: 6 1
extern "C" int printf(const char *, ...);
template <unsigned long... I> struct idx {};
template <class T, unsigned long N> struct R {};
template <class T, class U> constexpr bool nt = true;
template <class... T> struct tup {};
template <class C, class U> struct rs {
  using type = int;
};
template <class C, class U> using res_t = typename rs<C, U>::type;
template <class... T> struct tl {};

template <class Ix> struct conn;
template <unsigned long... I> struct conn<idx<I...>> {
  template <class S, class... C>
  static auto f(S &, C &&...) noexcept((nt<C, R<S, I>> && ...))
      -> tup<res_t<C, R<S, I>>...>;
  template <class S, class... C>
  static void g(S &, C &&...) noexcept((nt<C, R<S, I>> && ...));
  template <class S, class... C>
  static auto h(S &, C...) -> tup<tl<R<C, I>, R<S, I>>...>;
};
template <class... T> struct ty {
  template <class... U> static auto f(U...) -> tup<tl<T, U>...>;
};
struct B {
  B(int = 1, int = 2);
};
template <class T> auto nb(T t) -> decltype(t, new B);
int count();
int main() {
  int s = 0;
  conn<idx<0, 1>>::f(s, 1, 2.0);
  conn<idx<0, 1>>::g(s, 1, 2.0);
  conn<idx<0, 1>>::h(s, 1, 2.0);
  ty<int, char>::f(1, 2.0);
  B *b = nb(1);
  printf("%d %d\n", count() + s, b == nullptr);
}
