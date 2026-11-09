// Mangling differential: GNU `__extension__` in dependent signatures
// ([temp.over.link]/5; spec/abi.md "Mangling", WP-1029). clang keeps a
// `UO_Extension` node that mangles as its operand alone, so
// `decltype(__extension__ t)` is not the decltype of an id-expression
// (`DTfp_E`, not `Dtfp_E`), and `A<__extension__ N>` wraps its argument
// in `X…E` like any non-primary expression.
struct M { int m; };
template <int N> struct A {};
template <int N> void f(A<__extension__ N>);
template <int N> void g(A<__extension__ (N + 1)>);
template <int N> void h(A<(__extension__ N)>);
template <class T> auto e1(T t) -> decltype(__extension__ t);
template <class T> auto e2(T t) -> decltype((__extension__ t));
template <class T> auto e3(T t) -> decltype(__extension__ (t));
template <class T> auto e4(T t) -> decltype(sizeof(__extension__ t));
template <class T> auto e5(T t) -> decltype(__extension__ t.m);
template <class T> auto e6(T t) -> decltype(-__extension__ t);
template <class T> auto e7(T t) -> A<sizeof(__extension__ t)> { return {}; }
template <class T> auto e8(T t) -> decltype(__extension__ __extension__ t);
template <class T> auto e9(T t) -> decltype(&__extension__ t);
template <class T> auto e10(T t) -> decltype(__extension__ (t + 1));

void use() {
  M mm{};
  f<1>({});
  g<1>({});
  h<1>({});
  int i = 0;
  e1(i);
  e2(i);
  e3(i);
  e4(i);
  e5(mm);
  e6(i);
  e7(i);
  e8(i);
  e9(i);
  e10(i);
}
