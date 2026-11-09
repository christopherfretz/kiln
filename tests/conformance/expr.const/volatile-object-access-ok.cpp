// [expr.const]/10.8, [class.ctor.general]/5, [class.dtor]/16: a volatile
// object that is never read nor written through is fine in a constant
// expression, and its constructor and destructor run as if it were not
// volatile (a volatile object's volatility starts once its construction
// completes). Valid twin of volatile-object-access.cpp. WP-1019.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
struct C {
  int x;
  constexpr C() : x(1) { x += 2; }
  constexpr ~C() { x = 9; }
  constexpr int get() const { return x; }
};
constexpr int unused() {
  volatile int v = 1;
  volatile int a[2] = {1, 2};
  return 2;
}
static_assert(unused() == 2);
constexpr bool address() {
  volatile int v = 1;
  volatile int *p = &v;
  const volatile int *q = p;
  return p == &v && q == p && sizeof(v) == sizeof(int);
}
static_assert(address());
constexpr int structors() {
  volatile C c;
  auto *p = new volatile C;
  delete p;
  const volatile C cc;
  return 4;
}
static_assert(structors() == 4);
constexpr int heap_array() {
  auto *p = new volatile int[3];
  bool ok = p + 3 != p;
  delete[] p;
  return ok ? 5 : 0;
}
static_assert(heap_array() == 5);
constexpr int non_volatile_view() {
  int x = 6;
  volatile int &r = x;
  return x + (&r == &x);
}
static_assert(non_volatile_view() == 7);
