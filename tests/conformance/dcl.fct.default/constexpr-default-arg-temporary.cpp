// [intro.execution]/5, [class.temporary]/6.9, [expr.const]: a temporary
// created in a default argument lives until the end of the
// full-expression containing the call, so the callee may read it.
struct X { int v = 4; constexpr X() {} constexpr ~X() { v = -1; } };
constexpr int g(const X &x = X()) { return x.v; }
static_assert(g() == 4);
struct T { int v; constexpr T(const X &x = X()) : v(x.v) {} };
static_assert(T().v == 4);
constexpr int h() { T a[2]; return a[0].v + a[1].v; }
static_assert(h() == 8);
