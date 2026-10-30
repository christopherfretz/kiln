// [class.union]/2, /6, [expr.const]/10.9: constant evaluation tracks the
// active member of a union; a constructor's mem-initializer or an
// assignment to a member of non-class type makes it active, also through
// an anonymous union; the active member is accessible through a pointer to
// member.
union U { int i; float f; constexpr U() : i(1) {} constexpr U(float x) : f(x) {} };
constexpr int a() { U u; return u.i; }
static_assert(a() == 1);
constexpr float b() { U u(2.0f); return u.f; }
static_assert(b() == 2.0f);
constexpr int d() { U u; u.f = 3.0f; u.i = 4; return u.i; }
static_assert(d() == 4);
struct S { int x; constexpr S(int v) : x(v) {} constexpr ~S() {} };
union V { S s; int n; constexpr V() : n(0) {} constexpr ~V() {} };
constexpr int e() { V v; return v.n; }
static_assert(e() == 0);
struct W { union { int p; char q; }; int k; constexpr W() : p(5), k(1) {} };
constexpr int g() { W w; w.q = 'a'; return w.q + w.k; }
static_assert(g() == 98);
union P { int x; long y; };
constexpr int via_member_pointer() { P u{1}; int P::*m = &P::x; u.*m = 6; return u.x; }
static_assert(via_member_pointer() == 6);
constexpr U copy() { U u(5.0f); U v = u; return v; }
static_assert(copy().f == 5.0f);
