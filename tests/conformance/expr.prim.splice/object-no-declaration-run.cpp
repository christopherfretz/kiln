// [expr.prim.splice]/2 (P2996R13), [meta.reflection.array] (P3491R3): a
// splice of a reflect_constant_string / reflect_constant_array result is
// an lvalue designating that template parameter object, a const array no
// declaration names: it subscripts, decays to the pointer
// define_static_string / define_static_array return, has the array's
// size, and ranges in a range-for, in constant expressions and through
// codegen. GCC 16.1 compiles and runs the same file with the same output.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: hello 6 10 20 3
//@ stdout: 10 20 30 1 1
#include <cstdio>
#include <meta>
#include <type_traits>
#include <vector>

constexpr auto rs = std::meta::reflect_constant_string("hello");
constexpr auto ra = std::meta::reflect_constant_array(std::vector<int>{10, 20, 30});

static_assert(std::is_same_v<decltype(([:ra:])), const int (&)[3]>);
static_assert(std::is_same_v<decltype(([:rs:])), const char (&)[6]>);
static_assert(sizeof([:rs:]) == 6 && [:rs:][1] == 'e' && [:rs:][5] == '\0');
static_assert([:ra:][0] + [:ra:][2] == 40);
static_assert(&[:rs:][0] == std::define_static_string("hello"));
static_assert(&[:ra:][0] == std::define_static_array(std::vector<int>{10, 20, 30}).data());
constexpr const char* cp = [:rs:];
static_assert(cp[4] == 'o');

consteval int sum() {
  int s = 0;
  for (int x : [:ra:]) s += x;
  return s;
}
static_assert(sum() == 60);

const char* p = [:rs:];
const int* q = [:ra:];

int main() {
  std::printf("%s %zu %d %d %zu\n", p, sizeof([:rs:]), q[0], [:ra:][1],
              std::extent_v<std::remove_reference_t<decltype(([:ra:]))>>);
  for (int x : [:ra:]) std::printf("%d ", x);
  std::printf("%d %d\n", p == &[:rs:][0], q == std::define_static_array(std::vector<int>{10, 20, 30}).data());
}
