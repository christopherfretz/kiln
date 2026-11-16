// [meta.unary.prop] std::is_structural and [meta.reflection.traits]
// std::meta::is_structural_type: libstdc++ 16.1 declares both only when
// `__has_builtin(__builtin_is_structural)`, GCC's trait for a structural
// type ([temp.param]/7): a scalar or lvalue reference type, or a
// literal class whose bases and non-static data members are public,
// non-mutable and structural. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
#include <type_traits>

struct Pub { int a; const long b; int &r; };
struct Mut { mutable int m; };
class Priv { int p; };
struct NonLit { ~NonLit(); };
struct Der : Pub {};
enum E { e0 };

static_assert(__has_builtin(__builtin_is_structural));
static_assert(std::is_structural_v<int> && std::is_structural_v<const float>);
static_assert(std::is_structural_v<int *> && std::is_structural_v<int Pub::*>);
static_assert(std::is_structural_v<E> && std::is_structural_v<decltype(nullptr)>);
static_assert(std::is_structural_v<int &> && !std::is_structural_v<int &&>);
static_assert(std::is_structural_v<Pub> && std::is_structural_v<Der>);
static_assert(!std::is_structural_v<Mut> && !std::is_structural_v<Priv>);
static_assert(!std::is_structural_v<NonLit> && !std::is_structural_v<void>);
static_assert(std::meta::is_structural_type(^^Pub));
static_assert(!std::meta::is_structural_type(^^Mut));
static_assert(std::meta::is_structural_type(^^std::meta::info));
