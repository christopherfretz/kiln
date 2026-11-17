// [meta.reflection.result] reflect_function, [expr.const]/10 (P2280R4): a
// reference to a function whose initializer is unknown is constexpr-
// unknown: it binds an unspecified function, which is no valid template
// argument, so reflect_function of it throws instead of making the
// evaluation fail; reflect_object of a constexpr-unknown object reference
// likewise. A reference whose initializer is known reflects its referent.
// Found running GCC's C++ testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
extern int (&fref)();
consteval bool ok() {
  try {
    (void)std::meta::reflect_function(fref);
  } catch (std::meta::exception &) {
    return false;
  }
  return true;
}
static_assert(!ok());
extern int &iref;
consteval bool oko() {
  try {
    (void)std::meta::reflect_object(iref);
  } catch (std::meta::exception &) {
    return false;
  }
  return true;
}
static_assert(!oko());
int f();
int (&gref)() = f;
static_assert(std::meta::reflect_function(gref) == ^^f);
