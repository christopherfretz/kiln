// [dcl.constexpr]/6: __builtin_strcmp is usable in constant expressions, so
// the function can produce one; clang does not warn (-Winvalid-constexpr).
// From clang's CXX/class/class.compare/class.compare.default/p5.cpp. Found
// by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-ok
struct S {
  char buff[10] = {};
  constexpr bool operator==(const char *p) const { return __builtin_strcmp(p, buff) == 0; }
};
