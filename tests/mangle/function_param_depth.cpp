// Mangling differential: the `fL <L-1> p` nesting of a function parameter
// named in a `decltype` (Itanium ABI §5.1.6 <function-param>; clang
// `mangleFunctionParam`): the function types being mangled around the
// reference, less the parameter's function-scope depth (the prototype
// scopes enclosing its own, [basic.scope.param]), less one in a result
// type. WP-305: a parameter of a function type nested in the signature
// (`int (*)(T a, decltype(a))`) mangled one level too deep, and
// requires-expression parameter lists did not count as prototype scopes.
template <class T> void own(T t, decltype(t) u) {}
template <class T> auto ret(T t) -> decltype(t) { return t; }
template <class T> void outer(T a, int (*p)(decltype(a))) {}
template <class T> void nested(int (*p)(T a, decltype(a) b)) {}
template <class T> void twice(T a, int (*p)(int (*q)(decltype(a)))) {}
template <class T> auto fnret(T a) -> int (*)(decltype(a)) { return 0; }
template <class T> auto both(int (*p)(T a, decltype(a) b)) -> decltype(p) { return p; }
template <class T> void inner_ret(T a, auto (*p)(T b) -> decltype(a + b)) {}
template <class T> void deeper(int (*p)(T a, int (*)(decltype(a)))) {}
// Distinct canonical types: `decltype(a)` of an outer and of a nested `a`.
template <class T> void outer2(T a, int (*p)(T, decltype(a))) {}
template <class T> void nested2(T, int (*p)(T a, decltype(a))) {}
template <class U> struct A {
  template <class T> void h(int (*p)(T a, decltype(a) b, U)) {}
  template <class T> auto m(T t, decltype(t) u) -> decltype(u) { return u; }
};
template <class T> using F = int(T a, decltype(a));
template <class T> void alias(T, F<T>*) {}
// Requires-expression parameter lists are prototype scopes.
template <class T> requires requires(T a, decltype(a) b) { a + b; } void r1(T) {}
template <class T> void r2(T x) requires requires(T a, decltype(a) b) { a + b + x; } {}
template <class T> void r3(T x) requires requires(T a, decltype(x) b) { a + b; } {}
template <class T>
  requires requires(T a) { requires requires(decltype(a) c, decltype(c) d) { a + c + d; }; }
void r4(T) {}
template <class T>
void r5(T x) requires requires(T a, int (*p)(decltype(a) q, decltype(q))) { a + x; } {}
template <class T>
  requires requires(T a, int (*p)(decltype(a) q, decltype(q))) { a; }
void r6(T) {}
inline void lambdas() {
  auto l = [](auto x, decltype(x) y) {};
  l(1, 2);
  auto l2 = [](auto x, int (*q)(decltype(x) y, decltype(y))) {};
  int (*fii)(int, int) = nullptr;
  l2(1, fii);
}
int fi(int);
int fii(int, int);
int fiil(int, int, long);
int fp(int (*)(int));
int fip(int, int (*)(int));
void use() {
  own(1, 2);
  ret(1);
  outer(1, fi);
  nested(fii);
  twice(1, fp);
  fnret(1);
  both(fii);
  inner_ret(1, fi);
  deeper(fip);
  outer2(1, fii);
  nested2(1, fii);
  A<long> a;
  a.h<int>(fiil);
  a.m(1, 2);
  alias(1, fii);
  r1(1);
  r2(1);
  r3(1);
  r4(1);
  r5(1);
  r6(1);
  lambdas();
}
// Deduction against a target type substitutes the signature alone
// ([temp.deduct.funcaddr], [temp.explicit]/7): `t` there is a substituted
// parameter, not the pattern's.
template <class T> void xi(T t, decltype(t) u) {}
template void xi<int>(int, int);
template <class T> void xn(T t, void (*p)(decltype(t))) {}
template void xn(long, void (*)(long));
template <> void xi<char>(char, char) {}
void (*pxi)(short, short) = xi<short>;
void (*pxi2)(unsigned, unsigned) = &xi;
