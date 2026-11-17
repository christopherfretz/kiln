// [over.built]/16, [class.compare.default]/3 (P2996R13, CWG3153): for
// `std::meta::info` there is a built-in candidate `bool operator==(info,
// info)` (and `!=`), so a defaulted `==` of a class with a member of type
// `std::meta::info` compares it (not deleted), in a class template too,
// with a consteval constructor, and the candidate is found for an operand
// of class type converting to `std::meta::info`. Found running GCC's C++
// testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 has no built-in operator==(info, info) candidate for an operand of class type converting to std::meta::info
//@ expect: compile-ok
//@ strict-diags
#include <meta>
struct A {
  std::meta::info a = ^^int;
  bool operator==(const A &) const = default;
};
template <class T, T V> struct B {
  T b = V;
  consteval B() {}
  bool operator==(const B &) const = default;
};
struct Conv {
  std::meta::info r;
  consteval operator std::meta::info() const { return r; }
};
consteval bool t() {
  A x, y, z{^^long};
  B<std::meta::info, ^^char> c, d;
  return x == y && x != z && c == d && !(c != d) && Conv{^^int} == (^^int) &&
         (^^int) != Conv{^^long};
}
static_assert(t());
