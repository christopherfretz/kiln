// GNU `always_inline` and the other function attributes on a lambda's
// call operator inside a function template ([expr.prim.lambda.closure]/4,
// [temp.inst]/3): the instantiated lambda's call operator takes the
// pattern's attributes (clang `TransformLambdaExpr` → `InstantiateAttrs`),
// and for a generic lambda its specializations then take them from it,
// so at -O0 the always-inliner removes the call and the definition, as
// for a lambda outside a template. kiln rebuilt the call operator from
// its specifiers without attributes, leaving a call to
// `_ZZ2a3IiET_S0_ENKUlTyS0_E_clIiEEDaS0_` (libc++'s `__load_vector`
// lambdas in `std::mismatch`, found by the hod backend project; WP-1355).
// The three shapes: a non-generic lambda, an `auto` parameter, a template
// parameter list; the attribute after the parameter list (GNU) or after
// the introducer (C++23). `noinline`, `cold`, `hot` and `flatten` ride on
// the same path. A standard attribute after the parameter list appertains
// to the function type and is ignored there (`a7`, as clang).
template <class T> T a1(T c) {
  return [=](T x) __attribute__((always_inline)) { return x + c; }(c);
}
template <class T> T a2(T c) {
  return [=](auto x) __attribute__((always_inline)) { return x + c; }(c);
}
template <class T> T a3(T c) {
  return [=]<class U>(U x) __attribute__((always_inline)) { return x + c; }(c);
}
template <class T> T a4(T c) {
  return [=]<class U> [[gnu::always_inline]] (U x) { return x + c; }(c);
}
template <class T> T a5(T c) {
  return [=] [[clang::always_inline]] (auto x) { return x + c; }(c);
}
template <class T> T a6(T c) {
  return [=] [[gnu::always_inline]] (T x) { return x + c; }(c);
}
#pragma clang diagnostic ignored "-Wignored-attributes"
template <class T> T a7(T c) {
  return [=]<class U>(U x) [[gnu::always_inline]] { return x + c; }(c);
}
template <class T> T b1(T c) {
  return [=]<class U>(U x) __attribute__((noinline)) { return x + c; }(c);
}
template <class T> T b2(T c) {
  return [=]<class U> [[gnu::cold]] (U x) { return x + c; }(c);
}
template <class T> T b3(T c) {
  return [=] [[gnu::hot]] (auto x) { return x + c; }(c);
}
template <class T> T b4(T c) {
  return [=]<class U> [[gnu::flatten]] (U x) { return x + c; }(c);
}
// A lambda in a member function template of a class template.
template <class T> struct S {
  template <class V> T m(V v) {
    return [=]<class U>(U x) __attribute__((always_inline)) { return T(x + v); }(v);
  }
};
int use() {
  return a1(1) + a2(2) + a3(3) + a4(4) + a5(5) + a6(6) + a7(7) + b1(8) + b2(9) +
         b3(10) + b4(11) + S<long>().m(12);
}
