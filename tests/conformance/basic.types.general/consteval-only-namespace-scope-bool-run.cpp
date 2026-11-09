// [basic.types.general] (P2996R13): a namespace-scope variable whose
// initializer compares or queries reflections is not of consteval-only
// type when its own type is not (a bool, an int, an array of bool, a
// pointer to char from define_static_string), so it is emitted with its
// constant value and may be odr-used at runtime; only the reflection
// operands stay in the constant evaluation (WP-505). A constexpr
// std::meta::info variable beside them is consteval-only and is never
// emitted. Compared with GCC 16.1.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: 1 1 1 1 1 0 1 1 0 y 1 1 1
#include <meta>
#include <array>
#include <cstdio>

int x;
long y;
constexpr bool b = std::meta::type_of(^^x) == ^^int;
inline constexpr bool b2 = type_of(^^y) != ^^int;
constexpr auto r = ^^x;
constexpr std::meta::info rs[] = {^^int, ^^long};
constexpr bool b3 = rs[1] == (^^long) && r == ^^x;
static constexpr int n = (int)nonstatic_data_members_of(
    ^^std::array<int, 1>, std::meta::access_context::unchecked()).size();
template <class T> constexpr bool is_ty = type_of(^^x) == ^^T;
struct S { static constexpr bool m = type_of(^^y) == ^^long; };
constexpr std::array<bool, 2> arr = {type_of(^^x) == ^^int, type_of(^^y) == ^^int};
constexpr const bool* pb = &b;
constexpr const char* cs = std::define_static_string(identifier_of(^^y));
const bool cb = type_of(^^x) == ^^int;
bool rb = [] { return type_of(^^x) == ^^int; }();

int main() {
  const bool* p = &b2;
  std::printf("%d %d %d %d %d %d %d %d %d %s %d %d %d\n", b, *p, b3, n,
              is_ty<int>, is_ty<long>, S::m, arr[0], arr[1], cs, *pb, cb, rb);
}
