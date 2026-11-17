// [meta.reflection.result] reflect_function, [expr.const]/10 (P2280R4): a
// reference to a function whose initializer is unknown is constexpr-
// unknown; reflect_function of it throws instead of making the
// evaluation fail. Found running GCC's C++ testsuite.
//@ xfail(kiln): WP-1614: reflect_function of a constexpr-unknown function reference fails the evaluation instead of throwing
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
