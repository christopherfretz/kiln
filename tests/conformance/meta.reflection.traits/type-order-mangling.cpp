// [meta.reflection.traits] (P2830R10): type_order(a, b) is a total order
// on types, implementation-defined. kiln gives GCC 16.1's
// (spec/reflection-traits.md): strcmp of the types' Itanium manglings
// (gcc/cp/method.cc type_order_value), so `int` (i) orders before
// `unsigned` (j) and `long` (l), a global class `Z` (1Z) before `int`, and
// `ns::B` (N2ns1BE) before `ns::Aa` (N2ns2AaE) though it spells after it,
// and `std::meta::info` (Dm) before `int`.
// A binding generator keeps its type sets sorted by type_order, so the
// order of what it emits follows it.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include <meta>
using namespace std::meta;

struct Z {};
namespace ns {
struct B {};
struct Aa {};
template <class T> struct T1 {};
}  // namespace ns
using I = int;

static_assert(type_order(^^int, ^^int) == 0);
static_assert(type_order(^^I, ^^int) == 0);
static_assert(type_order(^^int, ^^unsigned) < 0);
static_assert(type_order(^^int, ^^long) < 0);
static_assert(type_order(^^char, ^^int) < 0);
static_assert(type_order(^^Z, ^^int) < 0);
static_assert(type_order(^^ns::B, ^^ns::Aa) < 0);
static_assert(type_order(^^ns::Aa, ^^ns::B) > 0);
static_assert(type_order(^^ns::T1<int>, ^^ns::T1<char>) > 0);
static_assert(type_order(^^const int, ^^int) < 0);  // Ki < i
static_assert(type_order(^^int*, ^^int&) < 0);
// std::meta::info mangles as GCC's Dm (WP-909; it was kiln's u6__info, after
// i), whatever -fkiln-reflection-mangling says.
static_assert(type_order(^^info, ^^int) < 0);
static_assert(type_order(^^info, ^^Z) > 0);

int main() {}
