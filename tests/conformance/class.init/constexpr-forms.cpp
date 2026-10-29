// [class.init], [class.base.init]/6, [class.copy.ctor]/14, [dcl.fct.default],
// [expr.const]: constant evaluation of a delegating constructor, default
// arguments of constructors, the implicit copy constructor of a class with
// an array member (element-wise, through the elements' copy constructor),
// default construction of arrays of class type, and member calls on
// prvalues.
struct M { int v; constexpr M(int x = 2) : v(x) {} constexpr M(const M &o) : v(o.v + 10) {} };
struct A { M m[3]; int k; };
constexpr int copy_array() { A a{{M(1), M(2), M(3)}, 4}; A b = a; return b.m[0].v + b.m[2].v + b.k; }
static_assert(copy_array() == 28);
struct L { int a[4]; };
constexpr int copy_scalars() { L l{{1, 2, 3, 4}}; L c = l; return c.a[3]; }
static_assert(copy_scalars() == 4);
struct Del { int x, y; constexpr Del(int a, int b) : x(a), y(b) {} constexpr Del() : Del(7, 8) {} };
static_assert(Del().x == 7 && Del().y == 8);
constexpr int default_array() { M ms[2]; return ms[0].v + ms[1].v; }
static_assert(default_array() == 4);
struct V { int x; constexpr V(int a) : x(a) {} constexpr int get() const { return x; } };
static_assert(V(5).get() == 5);
static_assert(V{6}.x == 6);
