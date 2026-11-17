// [meta.reflection.extract]/3-6 (P2996R13): extract<T> of a variable of
// pointer type reads its value (through constant_of: a null pointer, a
// pointer to a variable, with a qualification conversion to `const T*`);
// extract<R(*)(A...)> of a variable holding a closure without captures
// converts it to the function pointer ([expr.prim.lambda.closure]/8), the
// conversion function a generic lambda does not have for a mismatched
// type making it an error. Found running GCC's C++ testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ strict-diags
#include <meta>
using namespace std::meta;
consteval bool throws(auto f) {
  try {
    f();
  } catch (std::meta::exception &) {
    return true;
  }
  return false;
}
constexpr int *p = nullptr;
static_assert(type_of(constant_of(^^p)) == ^^int *);
static_assert(extract<int *>(^^p) == nullptr);
static_assert(extract<const int *>(^^p) == nullptr);
constexpr int x = 1;
constexpr const int *q = &x;
static_assert(extract<const int *>(^^q) == &x && *extract<const int *>(^^q) == 1);
static_assert(throws([] { return extract<long *>(^^q); }));

constexpr auto lam = [](int v) { return v + 1; };
static_assert(extract<int (*)(int)>(^^lam)(2) == 3);
static_assert(throws([] { return extract<int (*)(long)>(^^lam); }));
const auto noarg = [] {};
static_assert(extract<void (*)()>(^^noarg) == noarg);
static_assert(throws([] { return extract<void (*)() noexcept>(^^noarg); }));
constexpr auto nx = []() noexcept { return 4; };
static_assert(extract<int (*)() noexcept>(^^nx)() == 4 && extract<int (*)()>(^^nx)() == 4);
