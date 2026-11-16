// [class.union]/1, [class.mem.general]/11, [expr.prim.this]/2: a default
// member initializer of a union member may name `this` and the address of
// another member; in a constant initializer `this` is the union being
// initialized. Found running GCC's C++ testsuite (WP-1468).
//@ std: c++20
//@ expect: compile-ok
union U1 { const char *p = &c; char c; };
constexpr U1 u1 = {};
static_assert(u1.p == &u1.c);
union U2 { int i; const U2 *self = this; };
constexpr U2 u2 = {};
static_assert(u2.self == &u2);
