// [dcl.init.general]/16, [temp.dep.expr]: `T x(a, b)` with a dependent type or argument is
// initialized at instantiation (libc++ `const basic_string __temp(__first, __last, __alloc())`)
//@ expect: run
//@ strict-diags
struct P {
  int v;
  P(int a, int b, int c) : v(a + b + c) {}
  int size() const { return v; }
};
template <class T> int f(const T& t) {
  auto n = t.size();
  const T r(1, 2, n);
  T s(t.size(), n, 0);
  return r.v + s.v;
}
template <class T> T twice(T t) { T r(t.v, t.v, 0); return r; }
int main() { return f(P(1, 2, 3)) == 21 && twice(P(2, 0, 0)).v == 4 ? 0 : 1; }
