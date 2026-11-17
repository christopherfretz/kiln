// [meta.reflection.define.aggregate]/5 (P3795): data_member_spec's
// annotations each reflect a value or object of a non-array object type
// (a namespace, a function, an array, a reference or a type is an error),
// and need a name (an unnamed bit-field has no annotations). An accepted
// description carries its annotations into the member define_aggregate
// declares, and its own display and comparison see them. Found running
// GCC's C++ testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ strict-diags
#include <meta>
using namespace std::meta;
consteval bool ok(info type, data_member_options opts) {
  try {
    (void)data_member_spec(type, opts);
  } catch (std::meta::exception &) {
    return false;
  }
  return true;
}
void fn();
constexpr int arr[1] = {1};
constexpr int i = 42;
struct S { int m; };
constexpr S s{7};
static_assert(!ok(^^int, {.name = "a", .annotations = {^^::}}));
static_assert(!ok(^^int, {.name = "a", .annotations = {^^fn}}));
static_assert(!ok(^^int, {.name = "a", .annotations = {^^arr}}));
static_assert(!ok(^^int, {.name = "a", .annotations = {^^int}}));
static_assert(!ok(^^int, {.bit_width = 0, .annotations = {^^i}}));
static_assert(!ok(^^int, {.bit_width = 3, .annotations = {^^i}}));
static_assert(ok(^^int, {.name = "a", .annotations = {^^i}}));
static_assert(ok(^^int, {.name = "a", .annotations = {^^s, reflect_constant(3)}}));
static_assert(ok(^^int, {.name = "a", .bit_width = 3, .annotations = {^^i}}));
static_assert(ok(^^int, {.bit_width = 0}));

struct T;
consteval {
  define_aggregate(^^T, {data_member_spec(^^int, {.name = "x", .annotations = {^^i}})});
}
static_assert(annotations_of(^^T::x).size() == 1);
static_assert([:constant_of(annotations_of(^^T::x)[0]):] == 42);
