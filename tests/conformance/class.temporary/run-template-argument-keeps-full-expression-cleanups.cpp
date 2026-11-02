// [intro.execution]/5, [class.temporary]/4: a temporary is destroyed at the
// end of its full-expression even when a template argument (a
// constant-expression, [expr.const]/22) is converted while that
// full-expression is being built: an explicit argument of a class
// template-id or a call, a defaulted `enable_if_t<..., int> = 0`
// parameter substituted by overload resolution (libc++ shared_ptr's
// `shared_ptr(Y*, D)`; pointer_deleter.pass.cpp). The converted argument
// must not take the enclosing full-expression's pending cleanups. WP-296.
//@ expect: run
//@ stdout: 0 0 0 0 0 0 0 0
extern "C" int printf(const char*, ...);
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };

struct D {
  static int live;
  D(int) { ++live; }
  D(const D&) { ++live; }
  ~D() { --live; }
};
int D::live = 0;

template <int N, int M = 0> struct C { static constexpr int v = N + M; };
template <int N, class X> int f(X) { return N; }
template <class T> struct SP {
  template <class Y, class X, typename enable_if<sizeof(X) != 0, int>::type = 0>
  SP(Y*, X) {}
  ~SP() {}
};
template <class T> struct SQ {
  template <class Y, class X, int = 0> SQ(Y*, X) {}
  ~SQ() {}
};
int use(D) { return 0; }

int main() {
  { int r = use(D(3)) + C<1>::v; printf("%d ", D::live + r - 1); }
  { int r = use(D(3)) + C<1, 1>::v; printf("%d ", D::live + r - 2); }
  { int r = (D(3), C<4>::v); printf("%d ", D::live + r - 4); }
  { int r = f<1>(D(3)); printf("%d ", D::live + r - 1); }
  { SP<int> p(new int, D(3)); printf("%d ", D::live); }
  { SQ<int> p((int*)nullptr, D(3)); printf("%d ", D::live); }
  { SP<int> p = SP<int>(new int, D(3)); printf("%d ", D::live); }
  printf("%d\n", D::live);
}
