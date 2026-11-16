// [meta.reflection.extract]: extract of a reflected reference parameter
// is not a constant expression once its frame has returned (the
// reflection outlived the reference), when the type does not match
// (`extract<long &>` of an `int &`), or when it would drop a const.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:41 note "'^^r' cannot be extracted as a 'long &'"
//@ diag: 23:46 note "in call to 'bad_type(x)'"
//@ diag: 24:15 note "in call to 'call_bad()'"
//@ diag: 26:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:48 note "'^^r' cannot be extracted as a 'int &'"
//@ diag: 26:15 note "in call to 'bad_const(2)'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:10 note "'^^r' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 33:15 note "in call to 'dangling()'"
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
