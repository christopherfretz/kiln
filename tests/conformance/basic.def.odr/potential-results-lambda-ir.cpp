// [basic.def.odr]/5, [expr.prim.lambda.capture]/8: `ca.arr[j]`, `ca.v`,
// `carr[j]` and `cb.big[j]` name the constexpr locals without odr-using
// them, so `m` captures nothing and `l` captures only `ca` (odr-used by
// `&ca`). clang `EmitDeclRefLValue` reads a non-odr-used local of an
// enclosing function from `createUnnamedGlobalFrom`'s cached
// `__const.<function>.<variable>`, the global the variable's own
// initialization copies from; a constructor or destructor is spelled by
// its class name (spec/sema.md "Potential results", WP-1144).
// std: c++20 keeps the C++26 erroneous-value fill out of the IR.
//@ expect: ir
//@ std: c++20
struct A { int v; int arr[2]; };
struct B { int big[10]; };
int f(int i) {
  constexpr A ca{4, {5, 6}};
  constexpr B cb{{1, 2, 3, 4, 5, 6, 7, 8, 9, 10}};
  constexpr int carr[3] = {7, 8, 9};
  auto l = [=](int j) { return ca.v + ca.arr[j] + cb.big[j] + carr[j] + (&ca)->v; };
  auto m = [=](int j) { return ca.arr[j] + carr[j]; };
  return l(i) + m(i) + ca.arr[i];
}
// A lambda naming a constant local it never copied into a local of its
// own: the global is created on that use.
int g(int i) {
  static constexpr int unused = 0;
  constexpr int t[2] = {3, 4};
  return [](int j) { return t[j] + unused; }(i);
}
int h(const int *);
struct S {
  S() {
    constexpr int a[3] = {1, 2, 3};
    x = h(a) + [](int j) { return a[j]; }(x);
  }
  ~S() {
    const int b[3] = {4, 5, 6};
    h(b);
  }
  int x;
};
template <class T> struct U {
  U() {
    const T c[2] = {1, 2};
    h(c);
  }
};
void k() {
  S s;
  U<int> u;
}
