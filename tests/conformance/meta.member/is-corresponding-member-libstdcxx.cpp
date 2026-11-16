// [meta.member] through libstdc++'s <type_traits>: std::is_corresponding_member
// and std::is_pointer_interconvertible_with_class, and their feature-test
// macros ([version.syn]), which libstdc++ gates on __has_builtin of GCC's
// builtins. Found running GCC's C++ testsuite (WP-1472).
//@ skip(clang): clang 19/22 do not have the builtins libstdc++ gates on
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
//@ args: -stdlib=libstdc++
#include <type_traits>

#if __cpp_lib_is_layout_compatible < 201907L
#error "__cpp_lib_is_layout_compatible"
#endif
#if __cpp_lib_is_pointer_interconvertible < 201907L
#error "__cpp_lib_is_pointer_interconvertible"
#endif

struct A { int a; long b; };
struct B { int x; char y; };
static_assert(std::is_corresponding_member(&A::a, &B::x));
static_assert(!std::is_corresponding_member(&A::b, &B::y));
static_assert(std::is_pointer_interconvertible_with_class(&A::a));
static_assert(!std::is_pointer_interconvertible_with_class(&A::b));
static_assert(std::is_layout_compatible_v<A, A>);
