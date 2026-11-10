// [meta.reflection.names]/6-7 (P2996R13), [temp.arg.nontype]/4: the
// display_string_of of a class-type template argument (its template
// parameter object) or of a class value (reflect_constant) is the value as
// GCC 16.1 prints it in the specialization's name (spec/reflection.md
// §3.9, WP-1229): the type as display_string_of prints it, a `char` array
// member as a string literal, any other array with its type (`int [2]{5,
// 6}`), an empty class as `T()`, a union by its active member's value
// without a designator, a null pointer member as `0`.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include <meta>
#include <algorithm>
#include <cstddef>
#include <string_view>
using namespace std::meta;

template <auto V> struct W {};
template <std::size_t N> struct fixed_string {
  char v[N]{};
  constexpr fixed_string(const char (&s)[N]) { std::copy_n(s, N, v); }
};
template <fixed_string S> struct named_ {};
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
struct CH {
  char c;
};
namespace ns {
struct In {
  int v;
};
struct Deep {
  In i;
  double d;
};
}  // namespace ns

consteval std::string_view arg0(info r) {
  return display_string_of(template_arguments_of(r)[0]);
}

// The argument prints as the specialization's name spells it.
constexpr info named = ^^named_<fixed_string{"CopyInFiles"}>;
static_assert(display_string_of(named) == "named_<fixed_string<12>{\"CopyInFiles\"}>");
static_assert(arg0(named) == "fixed_string<12>{\"CopyInFiles\"}");
static_assert(arg0(^^named_<fixed_string{"ab"}>) != arg0(^^named_<fixed_string{"ac"}>));

// Several members, nested classes and arrays, unions, empty classes.
static_assert(arg0(^^W<Pt2{1, 2.5, Col::green}>) == "Pt2{1, 2.5e+0, Col::green}");
static_assert(display_string_of(^^W<Pt2{1, 2.5, Col::green}>) == "W<Pt2{1, 2.5e+0, Col::green}>");
static_assert(arg0(^^W<Nested{{4}, {5, 6}}>) == "Nested{Pt{4}, int [2]{5, 6}}");
static_assert(arg0(^^W<ns::Deep{{7}, 0.5}>) == "ns::Deep{ns::In{7}, 5.0e-1}");
static_assert(arg0(^^W<ns::In{7}>) == "ns::In{7}");
static_assert(arg0(^^W<NP{nullptr}>) == "NP{0}");
static_assert(arg0(^^W<U{.f = 2.0f}>) == "U{2.0e+0f}");
static_assert(arg0(^^W<U{.i = 3}>) == "U{3}");
static_assert(arg0(^^W<E0{}>) == "E0()");
static_assert(arg0(^^W<CH{'a'}>) == "CH{'a'}");

// A class value reflection.
static_assert(display_string_of(reflect_constant(ns::In{3})) == "ns::In{3}");
static_assert(display_string_of(reflect_constant(Pt2{1, 2.5, Col::green})) ==
              "Pt2{1, 2.5e+0, Col::green}");

int main() {}
