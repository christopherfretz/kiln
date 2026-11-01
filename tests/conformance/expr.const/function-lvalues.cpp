// [expr.const], [dcl.init.ref]/5, [expr.ref]/7.3.1: references to functions
// and a static member function named by a member access (`t.f`) evaluate
// to the function in a constant expression (libc++'s `__invoke` forwards
// `char_traits<C>::eq` by reference).
//@ std: c++26
struct Tr {
  static constexpr int add(int a, int b) { return a + b; }
};
constexpr int sub(int a, int b) { return a - b; }
template <class P> constexpr int call(P &&p) { return p(5, 3); }
static_assert(call(sub) == 2);
static_assert(call(Tr::add) == 8);
constexpr int h() {
  Tr t;
  return call(t.add);
}
static_assert(h() == 8);
constexpr int i() {
  Tr t;
  Tr *pt = &t;
  int (*p)(int, int) = pt->add;
  return p(1, 1);
}
static_assert(i() == 2);
constexpr int (&ref)(int, int) = sub;
static_assert(ref(9, 1) == 8);
constexpr int k(bool b) {
  int (&r)(int, int) = b ? sub : Tr::add;
  return r(4, 2);
}
static_assert(k(true) == 2 && k(false) == 6);
constexpr int m() {
  int (&r)(int, int) = sub;
  int (*p)(int, int) = &r;
  return p(7, 1);
}
static_assert(m() == 6);
