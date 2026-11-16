// [meta.reflection.extract] (extract-ref, extract-value): a reference
// parameter of a consteval function, whose lifetime began within the
// evaluation ([basic.life]/1), extracts to the object it refers to, as a
// value or as a reference (const included). GCC 16.1 rejects it, so no
// GCC oracle (spec/reflection-queries.md §5). Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
#include <meta>

consteval int by_param(int &r) { return std::meta::extract<int>(^^r); }
consteval int call_param() { int x = 3; return by_param(x); }
static_assert(call_param() == 3);

consteval void set_param(int &r) { std::meta::extract<int &>(^^r) = 7; }
consteval int call_set() { int x = 3; set_param(x); return x; }
static_assert(call_set() == 7);

consteval int by_cparam(const int &r) { return std::meta::extract<const int &>(^^r); }
static_assert(by_cparam(4) == 4);

consteval int outer(int &r) {
  auto inner = [](int &s) consteval { return std::meta::extract<int>(^^s) + 1; };
  return inner(r) + std::meta::extract<int>(^^r);
}
consteval int call_outer() { int x = 10; return outer(x); }
static_assert(call_outer() == 21);
