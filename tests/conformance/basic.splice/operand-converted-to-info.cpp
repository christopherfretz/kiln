// [basic.splice]/1: a splice operand is a converted constant expression
// of type std::meta::info, so a class operand converts to it through a
// consteval conversion function. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ oracle: gcc
#include <meta>

struct ToInt { consteval operator std::meta::info() const { return ^^int; } };
struct ToVar { consteval operator std::meta::info() const; };
constexpr int answer = 42;
consteval ToVar::operator std::meta::info() const { return ^^answer; }

constexpr ToInt ti{};
typename [: ti :] i = 7;
static_assert(sizeof(i) == sizeof(int));
static_assert([: ToVar{} :] == 42);

template <class R> constexpr int f() { return [: R{} :]; }
static_assert(f<ToVar>() == 42);
