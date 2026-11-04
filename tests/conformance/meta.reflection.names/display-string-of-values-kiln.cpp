// [meta.reflection.names]/6-7 (P2996R13): the display_string_of rows where
// kiln deliberately differs from GCC 16.1 (spec/reflection.md §3.9,
// WP-564). The string is implementation-defined; kiln's contract is a
// spelling that re-parses as the value, which GCC's sometimes is not
// (`& gv`, `(const int*)(& arr)`, `pt.Pt::x`, `0` for a null `int *`):
// floating point is the shortest round-tripping decimal (`1e-1`; GCC
// prints `1.0000000000000001e-1`), pointers are `&x` without GCC's space,
// a function pointer keeps its `&`, characters use the escapes of
// diagnostics (`'\n'`; GCC `'\012'`), `signed`/`unsigned char` print as
// character literals and `char8_t`/`wchar_t` with their prefix (GCC
// prints the code), a nested array as `{...}` without its type, a union
// with its designator. A value with no source spelling (an infinity, a
// closure object) keeps the `^^(value of type T)` placeholder.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

template <auto V> struct W {};
enum class Col { red = 2, green = 3 };
struct Pt {
  int x;
};
struct Pt2 {
  int x;
  double y;
  Col c;
};
struct Nested {
  Pt p;
  int a[2];
};
struct NP {
  const int* p;
};
union U {
  int i;
  float f;
};
struct E0 {};
constexpr Pt pt{3};
int gv;
const int arr[3] = {1, 2, 3};
void fn();
namespace ns {
int nv;
}
constexpr auto lam = [] {};

consteval std::string_view arg0(info r) {
  return display_string_of(template_arguments_of(r)[0]);
}

// Floating point.
static_assert(arg0(^^W<1.5>) == "1.5e0");
static_assert(arg0(^^W<1.5f>) == "1.5e0");
static_assert(arg0(^^W<0.1>) == "1e-1");
static_assert(arg0(^^W<100.0>) == "1e2");
static_assert(arg0(^^W<-2.0>) == "-2e0");
static_assert(display_string_of(reflect_constant(1.5)) == "1.5e0");

// Characters.
static_assert(arg0(^^W<'\n'>) == "'\\n'");
static_assert(arg0(^^W<(signed char)65>) == "'A'");
static_assert(arg0(^^W<(unsigned char)65>) == "'A'");
static_assert(arg0(^^W<u8'a'>) == "u8'a'");
static_assert(arg0(^^W<L'a'>) == "L'a'");

// Pointers.
static_assert(arg0(^^W<&gv>) == "&gv");
static_assert(arg0(^^W<&ns::nv>) == "&ns::nv");
static_assert(arg0(^^W<&fn>) == "&fn");
static_assert(arg0(^^W<arr>) == "&arr[0]");
static_assert(arg0(^^W<&arr[1]>) == "&arr[1]");
static_assert(arg0(^^W<(int*)nullptr>) == "nullptr");
static_assert(display_string_of(reflect_constant(&gv)) == "&gv");

// Class values.
static_assert(arg0(^^W<Pt2{1, 2.5, Col::green}>) == "Pt2{1, 2.5e0, Col::green}");
static_assert(arg0(^^W<Nested{{4}, {5, 6}}>) == "Nested{Pt{4}, {5, 6}}");
static_assert(arg0(^^W<NP{nullptr}>) == "NP{nullptr}");
static_assert(arg0(^^W<NP{&gv}>) == "NP{&gv}");
static_assert(arg0(^^W<U{.f = 2.0f}>) == "U{.f = 2e0}");
static_assert(arg0(^^W<E0{}>) == "E0{}");

// Subobjects and nested reflections.
static_assert(display_string_of(reflect_object(pt.x)) == "pt.x");
static_assert(arg0(^^W<reflect_constant(5)>) == "std::meta::reflect_constant(5)");
static_assert(arg0(^^W<reflect_object(gv)>) == "std::meta::reflect_object(gv)");

// No source spelling: the placeholder.
static_assert(arg0(^^W<__builtin_huge_val()>) == "^^(value of type double)");
static_assert(std::string_view(arg0(^^W<lam>).data(), 18) == "^^(object of type ");

int main() {}
