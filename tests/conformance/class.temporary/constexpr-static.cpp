// [class.temporary]/6, [expr.const]/4, /14: a temporary bound to a
// constexpr reference of static storage duration lives as long as the
// reference; a const one is usable in constant expressions, and it is a
// permitted result of a constant expression (pointers to it are constant).
struct S { int a, b; constexpr int sum() const { return a + b; } };
constexpr const int &r = 5;
static_assert(r == 5);
constexpr const S &s = S{1, 2};
static_assert(s.a == 1 && s.sum() == 3);
constexpr const int *p1 = &r, *p2 = &r;
static_assert(p1 == p2 && *p1 == 5);
constexpr int sq(const int &x) { return x * x; }
constexpr const int &q = sq(3);
static_assert(q == 9 && sq(q) == 81);
struct L { const int &ref; int k; };
constexpr L l{7, 1};
static_assert(l.ref == 7 && l.k == 1);
struct N { const S &s; int k; };
constexpr N n{S{1, 2}, 3};
static_assert(n.s.b == 2 && n.k == 3);
constexpr const int &ca = 4 + 1;
constexpr const int *pa[] = {&ca, &r};
static_assert(*pa[0] == 5 && *pa[1] == 5 && pa[0] != pa[1]);
