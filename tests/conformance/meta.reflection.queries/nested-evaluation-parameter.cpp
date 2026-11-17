// [expr.const] (CWG3162), [meta.reflection.queries] variable_of: a
// constant expression nested in an immediate function (a constexpr
// variable's initializer, a splice) may use variable_of of the enclosing
// function's own parameter, which is the function's current one, also
// through a function it calls there. Called directly in the enclosing
// evaluation (a static_assert at namespace scope), whose point of
// evaluation is outside the function, it throws. Found running GCC's C++
// testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
consteval int fn(int p) {
  constexpr auto rv = std::meta::variable_of(std::meta::parameters_of(^^fn)[0]);
  static_assert(rv == ^^p);
  return [:std::meta::variable_of(std::meta::parameters_of(^^fn)[0]):] + 1;
}
static_assert(fn(41) == 42);

consteval std::meta::info var(std::meta::info p) { return std::meta::variable_of(p); }
consteval int fn2(int q) {
  constexpr auto rv = var(std::meta::parameters_of(^^fn2)[0]);
  static_assert(rv == ^^q);
  return q;
}
static_assert(fn2(3) == 3);
consteval bool direct(int) {
  try {
    (void)std::meta::variable_of(std::meta::parameters_of(^^direct)[0]);
  } catch (std::meta::exception &) {
    return false;
  }
  return true;
}
static_assert(!direct(0));
consteval bool other(int) {
  constexpr bool ok = [] consteval {
    try {
      (void)std::meta::variable_of(std::meta::parameters_of(^^fn)[0]);
    } catch (std::meta::exception &) {
      return false;
    }
    return true;
  }();
  return ok;
}
static_assert(!other(0));
