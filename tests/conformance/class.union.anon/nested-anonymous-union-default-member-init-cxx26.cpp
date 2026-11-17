// [class.union.general]/2 under P3074R7 (C++26): an anonymous union member
// of a union is not a variant member (its members are), so a nested
// anonymous union whose member has a default member initializer keeps its
// non-trivial default constructor, and the enclosing union's default
// constructor runs it: the member is active through every level. The
// C++20 rules are nested-anonymous-union-default-member-init.cpp; clang 22
// and GCC 16.1 do not implement P3074R7.
//@ std: c++26
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
static_assert(!__is_trivially_constructible(W));
static_assert(__is_trivially_copyable(V) && __is_trivially_destructible(V));
static_assert(__is_aggregate(V) && __is_aggregate(U) && !__is_aggregate(C));
