// __builtin_is_corresponding_member on members of anonymous classes
// ([class.mem.general]/23, [meta.member]/4). An anonymous union is a
// member of the common initial sequence; its members are not, so they
// never correspond (GCC 16.1 gives "sorry, unimplemented: not well
// defined for anonymous unions"; kiln follows the text and answers
// false). The members of a corresponding pair of anonymous structs (a GNU
// extension, no text) correspond as in the anonymous structs' own
// sequence, as in GCC. Found running GCC's C++ testsuite (WP-1472).
//@ skip(clang): clang 19/22 do not have __builtin_is_corresponding_member
//@ strict-diags
//@ std: c++20
#define CM(a, b) __builtin_is_corresponding_member(a, b)

struct V1 { int a; union { int b; long c; }; int d; };
struct V2 { int a; union { long b; int c; }; int d; };
static_assert(CM(&V1::a, &V2::a));
static_assert(CM(&V1::d, &V2::d));
static_assert(!CM(&V1::b, &V2::c));
static_assert(!CM(&V1::c, &V2::b));

struct W1 { int a; struct { int b; short c; }; long d; };
struct W2 { int a; struct { int x; short y; }; long z; };
struct W3 { int a; struct { int x; int y; }; long z; };
static_assert(CM(&W1::b, &W2::x));
static_assert(CM(&W1::c, &W2::y));
static_assert(CM(&W1::d, &W2::z));
static_assert(!CM(&W1::b, &W2::y));
static_assert(!CM(&W1::b, &W3::x));  // the anonymous structs differ
static_assert(!CM(&W1::d, &W3::z));
