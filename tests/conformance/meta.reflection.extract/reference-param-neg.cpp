// [meta.reflection.extract]: extract of a reflected reference parameter
// is not a constant expression once its frame has returned (the
// reflection outlived the reference), when the type does not match
// (`extract<long &>` of an `int &`), or when it would drop a const.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:41 note "'^^r' cannot be extracted as a 'long &'"
//@ diag: 24:46 note "in call to 'bad_type(x)'"
//@ diag: 25:15 note "in call to 'call_bad()'"
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:48 note "'^^r' cannot be extracted as a 'int &'"
//@ diag: 27:15 note "in call to 'bad_const(2)'"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:10 note "'^^r' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 34:15 note "in call to 'dangling()'"
//@ diag: 28:37 note "declared here"
#include <meta>

consteval int bad_type(int &r) { return std::meta::extract<long &>(^^r); }
consteval int call_bad() { int x = 1; return bad_type(x); }
static_assert(call_bad() == 1);
consteval int bad_const(const int &r) { return std::meta::extract<int &>(^^r); }
static_assert(bad_const(2) == 2);
consteval std::meta::info leak(int &r) { return ^^r; }
consteval int dangling() {
  int x = 1;
  auto i = leak(x);
  return std::meta::extract<int>(i);
}
static_assert(dangling() == 1);
