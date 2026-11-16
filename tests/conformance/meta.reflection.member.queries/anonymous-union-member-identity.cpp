// [class.union.anon]/1: a member of an anonymous union named from the
// enclosing scope is that union's member: `^^a` and the element of
// `members_of` of the union are one reflection, and a member access
// or pointer to member through a splice of it reaches the member through
// the enclosing class. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
using namespace std::meta;
constexpr auto ctx = access_context::unchecked();

static union { int a; };
static_assert(^^a == members_of(parent_of(^^a), ctx)[0]);

struct S { int k; union { int b; long c; }; };
static_assert(^^S::b == nonstatic_data_members_of(parent_of(^^S::b), ctx)[0]);
static_assert(^^S::c == nonstatic_data_members_of(parent_of(^^S::c), ctx)[1]);
static_assert(parent_of(^^S::b) != ^^S);
static_assert(identifier_of(^^S::b) == "b");

constexpr S s0{1, {5}};
static_assert(s0.[: ^^S::b :] == 5);
consteval int set() { S s{1, {3}}; s.[: ^^S::b :] = 9; return s.b; }
static_assert(set() == 9);
constexpr int S::*pm = &[: ^^S::b :];
static_assert(s0.*pm == 5);
