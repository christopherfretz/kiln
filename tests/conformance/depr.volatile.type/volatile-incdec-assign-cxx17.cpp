// [depr.volatile.type]: before C++20 no increment, decrement, assignment
// or structured-binding use of a volatile object is deprecated (clang
// checks `CPlusPlus20`; WP-432).
//@ std: c++17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 17:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 18:14 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 19:23 warning "expression with side effects has no effect in an unevaluated context"
volatile int gv;
int f(int);
struct C { int a, b; };
void g(volatile int &r, volatile int *p, volatile C &vc) {
  r += 1;
  r |= 1;
  *p *= 2;
  (void)sizeof(r = 1);
  decltype(r = 1) q = r;
  bool b = noexcept(r = 1);
  (void)(r = 1);
  static_cast<void>(r = 9);
  (r = 1);
  ((r = 1));
  for (;; r = 1) break;
  r = 1 ? 2 : 3;
  vc.a = 3;
  int z = (r = 1);
  int w = (vc.a = 4);
  f(r = 2);
  if ((r = 5)) {}
  (void)(gv = gv = 1);
  r = r = 1;
  0 ? r = 1 : r = 2;
  auto l = [&] { return r = 7; };
  r++;
  --r;
  p[0]++;
  ++*p;
  gv--;
  vc.a++;
  int i = 0;
  i = r;
  i++;
  (void)q; (void)b; (void)z; (void)w; (void)l;
}
int ns = (gv = 1);
void h() {
  volatile auto [a0, b0] = C{1, 2};
  const volatile auto [a1, b1] = C{1, 2};
  auto volatile [a2, b2] = C{1, 2};
  const auto &[a3, b3] = C{1, 2};
}
template <class T> void t(T &x) { x++; x = x = 1; }
template void t(volatile int &);
template void t(int &);
