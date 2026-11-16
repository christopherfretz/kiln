// [meta.reflection.names]/1: whether a function parameter has an
// identifier depends on the declarations of the function that precede
// the evaluation point: a later redeclaration that names it (or names it
// differently) changes the answer, also through a consteval wrapper
// whose earlier call must not be reused. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
using namespace std::meta;

consteval bool named(info r) { return has_identifier(r); }

void f(int, int b, int c);
constexpr auto p0 = parameters_of(^^f)[0];
constexpr auto p1 = parameters_of(^^f)[1];
constexpr auto p2 = parameters_of(^^f)[2];
static_assert(!named(p0) && named(p1) && named(p2));
void f(int a, int, int d);
static_assert(named(p0) && named(p1) && !named(p2));
static_assert(has_identifier(p0) && identifier_of(p0) == "a");
static_assert(identifier_of(p1) == "b");
static_assert(!has_identifier(p2));
