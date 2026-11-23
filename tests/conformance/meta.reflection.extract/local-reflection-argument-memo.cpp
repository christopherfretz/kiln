// A reflection of an automatic variable or a parameter given to a
// function: `extract` of it there reads the variable's value at that
// call ([meta.reflection.extract]: its lifetime began within the
// evaluation), so a second call with the same reflection after the
// variable changed must read the new value, not a cached result of the
// first call (the reflection's bits are the same). Reduced from a test
// of evaluation-session keys. No GCC oracle: GCC 16.1 rejects `extract`
// of a variable of another function's frame ("reflection does not
// represent an annotation or a valid argument to a splice-expression").
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include <meta>

using std::meta::info;

consteval int read(info r) { return std::meta::extract<int>(r); }
consteval int pass(info r) { return read(r) + 0; }

consteval int local() {
  int x = 1;
  int a = read(^^x);
  x = 5;
  int b = read(^^x);
  return a * 10 + b;
}
static_assert(local() == 15);

consteval int through_a_caller() {
  int x = 2;
  int a = pass(^^x);
  x = 7;
  return a * 10 + pass(^^x);
}
static_assert(through_a_caller() == 27);

consteval int param(int x) {
  int a = read(^^x);
  x += 3;
  return a * 10 + read(^^x);
}
static_assert(param(4) == 47);
