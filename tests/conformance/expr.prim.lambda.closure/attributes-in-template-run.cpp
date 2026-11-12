// [expr.prim.lambda.closure]/4, [temp.inst]/3: a lambda in a function
// template keeps its call operator's attributes when the template is
// instantiated, for a generic lambda in each specialization of the new
// call operator too (clang `TransformLambdaExpr` → `InstantiateAttrs`).
// At -O0 the `always_inline` operators are inlined and removed; the
// values are unchanged. kiln dropped the attributes (WP-1355).
//@ expect: run
//@ strict-diags
//@ stdout: 3 5 7 9 11
extern "C" int printf(const char *, ...);

template <class T> T f1(T c) {
  return [=](T x) __attribute__((always_inline)) { return x + c; }(c + 1);
}
template <class T> T f2(T c) {
  return [=](auto x) __attribute__((always_inline)) { return x + c; }(c + 1);
}
template <class T> T f3(T c) {
  return [=]<class U>(U x) __attribute__((always_inline)) { return x + c; }(c + 1);
}
template <class T> T f4(T c) {
  return [=]<class U> [[gnu::noinline]] (U x) { return x + c; }(c + 1);
}
template <class T> struct S {
  template <class V> T m(V v) {
    return [=]<class U> [[gnu::always_inline]] (U x) { return T(x + v); }(v + 1);
  }
};
int main() {
  printf("%d %d %d %d %ld\n", f1(1), f2(2), f3(3), f4(4), S<long>().m(5));
}
