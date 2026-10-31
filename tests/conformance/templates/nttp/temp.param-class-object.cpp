// [temp.param]/8: a non-type template parameter of class type names a
// template parameter object, one per (type, value); it is an lvalue of type
// `const T`, usable in constant expressions.
//@ expect: compile-ok
struct P { int x, y; };
template <P p> constexpr int sum() { return p.x + p.y; }
static_assert(sum<P{1, 2}>() == 3);
template <P p> struct S { static constexpr int v = p.x * 10 + p.y; };
static_assert(S<P{3, 4}>::v == 34);
template <P p> constexpr const P* addr() { return &p; }
static_assert(addr<P{1, 2}>() == addr<P{1, 2}>());
static_assert(addr<P{1, 2}>() != addr<P{2, 1}>());
static_assert(addr<P{1, 2}>()->y == 2);
template <P p> constexpr const P& ref() { return p; }
static_assert(&ref<P{5, 6}>() == addr<P{5, 6}>());
template <P p> constexpr bool is_const_lvalue() { return __is_same(decltype((p)), const P&); }
static_assert(is_const_lvalue<P{0, 0}>());
template <auto v> constexpr auto id() { return v; }
static_assert(id<P{7, 8}>().y == 8);
struct Q { int a[3]; double d; };
template <Q q> constexpr double qq() { return q.a[0] + q.a[2] + q.d; }
static_assert(qq<Q{{1, 2, 3}, 0.5}>() == 4.5);
static_assert(__is_same(S<P{1, 2}>, S<P{1, 2}>));
static_assert(!__is_same(S<P{1, 2}>, S<P{2, 1}>));
union U { int i; float f; };
template <U u> constexpr int ui() { return u.i; }
static_assert(ui<U{9}>() == 9);
struct D { int x; constexpr D(int v) : x(v * 2) {} };
template <D d> constexpr int dx() { return d.x; }
static_assert(dx<D(4)>() == 8);
static_assert(dx<5>() == 10);
