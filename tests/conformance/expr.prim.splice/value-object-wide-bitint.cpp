// [expr.prim.splice]/2, [meta.reflection.result], [meta.reflection.queries]
// (P2996R13) for a _BitInt wider than 128 bits, a structural type
// ([temp.param]/7: a scalar): reflect_constant of one (of a variable, a
// local of a consteval function, a prvalue) is a value reflection,
// constant_of of a member or array element of one is its value, and each
// splices to a prvalue of the value (WP-1743). x86-64 only: the AArch64
// ABI caps _BitInt at 128 bits. GCC 16.1 has no _BitInt in C++.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ requires: target-x86_64
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

struct W { int k; _BitInt(200) b; unsigned _BitInt(129) u; };
constexpr W w{1, -5, 7};
constexpr _BitInt(200) arr[2] = {3, (_BitInt(200))1 << 150};
constexpr _BitInt(200) x = 6;

constexpr auto c1 = constant_of(reflect_object(w.b));
constexpr auto c2 = constant_of(reflect_object(w.u));
constexpr auto c3 = constant_of(reflect_object(arr[1]));
constexpr auto c4 = constant_of(reflect_object(arr));
static_assert(is_value(c1) && type_of(c1) == ^^_BitInt(200));
static_assert(type_of(c2) == ^^unsigned _BitInt(129));
static_assert([:c1:] == -5);
static_assert([:c2:] == 7);
static_assert([:c3:] == (_BitInt(200))1 << 150);
static_assert([:c4:][0] == 3);

constexpr auto v1 = reflect_constant(x);
consteval info local() { _BitInt(200) y = 7; return reflect_constant(y); }
constexpr auto v3 = reflect_constant(~(unsigned _BitInt(300))0);
static_assert([:v1:] == 6);
static_assert([:local():] == 7);
static_assert(([:v3:] >> 290) == 1023);
static_assert(v1 == constant_of(^^x));

template <_BitInt(200) V> constexpr _BitInt(200) tv = V;
static_assert(tv<([:c1:])> == -5);
