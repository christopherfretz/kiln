// [class.union.general]/2: the members of an anonymous union that is a
// member of a union are variant members of the enclosing union, so a
// default member initializer inside a nested anonymous union is "a
// variant member with a default member initializer" of the enclosing one
// ([class.default.ctor]/2 before C++26): its default constructor is not
// deleted and activates that member through every level. Also: an
// anonymous struct in an anonymous union, a constructor whose
// mem-initializer names another variant member (the default member
// initializer is then ignored, [class.base.init]/9), aggregate and
// designated initialization, and the traits. Reading another member is a
// read of an inactive member (constexpr-nested-anonymous-union-errors.cpp).
// GCC 16.1 initializes the first member for `V{}`; [dcl.init.aggr]/5.5
// names the member with the default member initializer, as clang does.
// Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
struct V { union { union { int x{5}; char y; }; long long z; }; };
struct W { union { union { union { int x{7}; char y; }; short s; }; long long z; }; };
struct A { union { struct { int a{1}; int b{2}; }; long long z; }; };
struct C {
  union { union { int x{5}; char y; }; long long z; };
  constexpr C() : z(9) {}
};
union U { union { int x{5}; char y; }; long long z; };

constexpr V v{};
static_assert(v.x == 5);
constexpr W w{};
static_assert(w.x == 7);
constexpr A a{};
static_assert(a.a == 1 && a.b == 2);
constexpr C c;
static_assert(c.z == 9);
constexpr U u{};
static_assert(u.x == 5);

constexpr int default_init_v() { V t; return t.x; }
static_assert(default_init_v() == 5);
constexpr int default_init_w() { W t; return t.x; }
static_assert(default_init_w() == 7);
constexpr int default_init_a() { A t; return t.a + t.b; }
static_assert(default_init_a() == 3);
constexpr int default_init_u() { U t; return t.x; }
static_assert(default_init_u() == 5);
// The nested union is active, so assigning its other member switches it.
constexpr int switch_inner() { V t; t.y = 'a'; return t.y; }
static_assert(switch_inner() == 'a');
constexpr long long switch_outer() { V t; t.z = 11; return t.z; }
static_assert(switch_outer() == 11);

constexpr V vz{.z = 3};
static_assert(vz.z == 3);
constexpr U uz{.z = 4};
static_assert(uz.z == 4);
constexpr V vy{{{.y = 2}}};
static_assert(vy.y == 2);
constexpr U uy{.y = 6};
static_assert(uy.y == 6);

static_assert(__is_constructible(V) && __is_constructible(W));
static_assert(__is_constructible(A) && __is_constructible(U));
static_assert(!__is_trivially_constructible(V) && !__is_trivially_constructible(U));
static_assert(!__is_trivially_constructible(W) && !__is_trivially_constructible(A));
static_assert(__is_trivially_copyable(V) && __is_trivially_destructible(V));
static_assert(__is_aggregate(V) && __is_aggregate(U) && !__is_aggregate(C));
