// [basic.lookup.argdep]/3.1 (P2996R13): the associated set of entities of
// std::meta::info is the enumeration std::meta::operators, so std::meta is
// an associated namespace of a reflection argument and unqualified calls
// of the std::meta functions find them by ADL, without a
// using-directive. An argument that is not a reflection associates
// nothing with std::meta. GCC 16.1 compiles the same file.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: m int 2
#include <meta>
#include <cstdio>

struct S { int m; static long sm; };
constexpr auto r = ^^S::m;

static_assert(display_string_of(type_of(r)) == "int");
static_assert(identifier_of(r) == "m");
static_assert(is_nonstatic_data_member(r));
static_assert(type_of(^^S::sm) == ^^long);
// A pointer or array of reflections associates std::meta too.
constexpr std::meta::info rs[] = {^^int, ^^S};
static_assert(is_type(rs[1]));
consteval std::size_t count(const std::meta::info* p) { return p ? 2 : 0; }

// A user function of a reflection is found alongside by ordinary lookup.
consteval int kind(std::meta::info x) { return is_type(x) ? 1 : 2; }
static_assert(kind(r) == 2);

namespace N {
struct T {};
consteval int g(T, std::meta::info) { return 7; }
}  // namespace N
static_assert(g(N::T{}, ^^int) == 7);

int main() {
  std::printf("%s %s %d\n", identifier_of(r).data(),
              display_string_of(type_of(r)).data(), (int)count(rs));
}
