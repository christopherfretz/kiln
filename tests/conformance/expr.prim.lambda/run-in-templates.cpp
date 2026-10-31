// [temp.inst]/3, [expr.prim.lambda.closure]: lambdas in function templates,
// class template members and member templates at run time: captures by
// copy, by reference, init-captures, this and *this, nested lambdas, a
// returned closure, and the conversion to pointer to function.
//@ expect: run
//@ stdout: 42 3 / 6 / 30 / 4 5 6 / 10 / 12 / 5 / 20 30
extern "C" int printf(const char *, ...);
template <class T> T twice(T v) { return [](T x) { return x + x; }(v); }
template <class T> T addk(T v, T k) {
  auto f = [k](T x) { return x + k; };
  auto g = [&k](T x) { k += x; return k; };
  g(v);
  return f(v) + k;
}
template <class T> T initcap(T v) {
  auto f = [y = v * 2, &r = v](T x) mutable { y += x; r += 1; return y; };
  T a = f(1);
  return a + f(1) + v;
}
template <class T> struct Box {
  T val;
  T get() const { return [this] { return val; }(); }
  T geti() const { return [=, this] { return val + 1; }(); }
  T getc() const { return [*this] { return val + 2; }(); }
};
struct S {
  int m = 7;
  template <class T> int mem(T t) { return [&] { return m + t; }(); }
};
template <class T> T nested(T v) {
  return [v] { return [v](T y) { return v * y; }(3); }();
}
template <class T> auto make(T v) { return [v](T y) { return v - y; }; }
template <class T> T fnptr(T v) {
  T (*p)(T) = [](T x) { return x * 10; };
  return p(v);
}
int main() {
  printf("%d %g / ", twice(21), twice(1.5));
  printf("%d / ", addk(1, 2));
  printf("%d / ", initcap(5));
  Box<int> b{4};
  printf("%d %d %d / ", b.get(), b.geti(), b.getc());
  S s;
  printf("%d / ", s.mem(3));
  printf("%d / ", nested(4));
  printf("%d / ", make(9)(4));
  printf("%d %ld\n", fnptr(2), fnptr(3L));
}
