// [meta.reflection.names]/6-7 (P2996R13): the display_string_of rows where
// kiln deliberately differs from GCC 16.1 (spec/reflection.md §3.9,
// WP-564). The string is implementation-defined; kiln's contract for a
// scalar is a spelling that re-parses as the value, which GCC's sometimes
// is not (`& gv`, `(const int*)(& arr)`, `pt.Pt::x`, `0` for a null
// `int *`): pointers are `&x` without GCC's space, a function pointer
// keeps its `&`, characters use the escapes of diagnostics (`'\n'`; GCC
// `'\012'`), `signed`/`unsigned char` print as character literals and
// `char8_t`/`wchar_t` with their prefix (GCC prints the code). Floating
// point, class values and nested reflections print as GCC prints them
// (WP-1229; display-string-of-floating.cpp, -class-args.cpp,
// -reflection-args.cpp), except a pointer member, which keeps kiln's
// pointer spelling. A value with no source spelling (a closure object)
// keeps the `^^(object of type T)` placeholder.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ xfail(gcc): the rows where kiln's display_string_of deliberately differs from GCC 16.1's (spec/reflection.md §3.9)
#include "kiln_meta.h"
using namespace std::meta;

template <auto V> struct W {};
struct Pt {
  int x;
};
struct NP {
  const int* p;
};
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

// Class values: a pointer member keeps kiln's pointer spelling (GCC
// `NP{((const int*)(& gv))}`).
static_assert(arg0(^^W<NP{&gv}>) == "NP{&gv}");

// Subobjects.
static_assert(display_string_of(reflect_object(pt.x)) == "pt.x");

// No source spelling: the placeholder.
static_assert(std::string_view(arg0(^^W<lam>).data(), 18) == "^^(object of type ");

int main() {}
