// spec/reflection.md §3.5: the class-typed results of the reflection
// functions, built by the Oven in the caller's result object by layout:
// string_view and u8string_view (a pointer into a frozen string and its
// length), source_location (a pointer to its __impl), vector<info>
// (allocated with the vector's allocator, owned and freed by the caller),
// member_offset and access_context. Exercised through the stand-in
// classes of tests/support/kiln_meta.h.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

struct S {
  int a;
};

// string_view / u8string_view: not NUL-terminated by contract, but the
// frozen string is; the same text is the same object.
static_assert(identifier_of(^^S).size() == 1 && identifier_of(^^S).data()[1] == 0);
static_assert(identifier_of(^^S).data() == identifier_of(^^S).data());
static_assert(u8identifier_of(^^S::a) == u8"a");

// source_location
static_assert(source_location_of(^^S).line() == 15);
static_assert(source_location_of(^^S).column() == 8);

// vector<info>: a fresh allocation per call, freed by ~vector.
consteval bool vec() {
  info arr[3] = {^^int, ^^S, ^^S::a};
  std::vector<info> v = __kiln_reflection_range(arr);
  std::vector<info> w = __kiln_reflection_range(arr);
  return v.size() == 3 && v[1] == (^^S) && v[2] == (^^S::a) && v.data() != w.data();
}
static_assert(vec());
consteval bool nested() {
  info none[1] = {^^int};
  std::vector<info> v = __kiln_reflection_range(__kiln_reflection_range(none));
  return v.size() == 1 && !v.empty();
}
static_assert(nested());

// member_offset
constexpr member_offset mo = __kiln_member_offset(4, 3);
static_assert(mo.bytes == 4 && mo.bits == 3 && mo.total_bits() == 35);

// access_context
consteval bool ctx() {
  access_context c = __kiln_access_context(^^S, ^^int);
  return c.scope() == (^^S) && c.designating_class() == ^^int;
}
static_assert(ctx());

int main() {}
