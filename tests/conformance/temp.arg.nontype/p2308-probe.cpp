// P2308R1 ([temp.arg.general]/1, [temp.param]/12): a braced-init-list is a
// template-argument and a default template-argument, and it
// copy-list-initializes the non-type template parameter: scalars, pointers
// (`{}` is null), class types (aggregate and constructor), a class template
// placeholder (deduced from the list), designated initializers, and the
// dependent forms, substituted at instantiation.
//@ expect: compile-ok
//@ strict-diags
struct A { int x, y; };
struct B { int v; constexpr B(int i) : v(i) {} };
template <class T> struct W { T t; };
template <A a> constexpr int f() { return a.x * 10 + a.y; }
template <int I = {}> constexpr int g() { return I; }
template <A a = {3, 4}> constexpr int h() { return a.x + a.y; }
template <class T, T v = {}> constexpr int k() { return (int)v; }
template <B b> constexpr int m() { return b.v; }
template <int *p = {}> struct Z { static constexpr bool null = p == nullptr; };
template <W w> constexpr int n() { return w.t; }
template <class T> constexpr int dep() { return f<{T(1), T(2)}>() + g<{T(5)}>(); }
template <A a> struct C { static constexpr int v = a.x * 10 + a.y; };
static_assert(f<{1, 2}>() == 12);
static_assert(g<>() == 0 && g<{5}>() == 5);
static_assert(h() == 7);
static_assert(k<long>() == 0 && k<int, {9}>() == 9);
static_assert(m<{8}>() == 8);
static_assert(Z<>::null);
static_assert(n<{6}>() == 6);
static_assert(dep<int>() == 17);
static_assert(C<{.x = 4, .y = 1}>::v == 41);
static_assert(C<{.y = 2}>::v == 2);
