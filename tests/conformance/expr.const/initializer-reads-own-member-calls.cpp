// [expr.const]/10.9, [class.cdtor]/2: a function called during a
// variable's initialization that names the variable sees the object under
// construction as it is at that point, each time it is called (a call that
// reads it is not memoized across the object's changes). WP-1100.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
struct C {
  int x, y;
  constexpr C();
};
extern const C c;
constexpr int get(int) { return c.x; }
constexpr C::C() : x(1), y(0) {
  int a = get(0);
  x = 2;
  int b = get(0);
  y = a * 10 + b;
}
constexpr C c;
static_assert(c.y == 12);
struct E { int x, y, z; };
extern const E e;
constexpr int gx(int k) { return e.x + k; }
constexpr E e{5, gx(1), gx(2)};
static_assert(e.y == 6 && e.z == 7);
constexpr int k = gx(3);
static_assert(k == 8);
