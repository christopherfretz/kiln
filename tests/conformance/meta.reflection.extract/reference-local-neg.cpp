// [meta.reflection.extract]: extract of a reflected block-scope reference
// is not a constant expression once its block has ended (the reflection
// escaped the block), once its frame has returned, when the type does not
// match (`extract<long &>` of an `int &`), or when it would drop a const.
// GCC 16.1 rejects all of these too. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:10 note "'^^r' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 35:15 note "in call to 'escaped()'"
//@ diag: 30:10 note "declared here"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:10 note "'^^r' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 45:15 note "in call to 'returned()'"
//@ diag: 38:8 note "declared here"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:10 note "'^^r' cannot be extracted as a 'long &'"
//@ diag: 52:15 note "in call to 'bad_type()'"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:10 note "'^^r' cannot be extracted as a 'int &'"
//@ diag: 59:15 note "in call to 'bad_const()'"
#include <meta>

consteval int escaped() {
  int x = 1;
  std::meta::info i;
  {
    int &r = x;
    i = ^^r;
  }
  return std::meta::extract<int>(i);
}
static_assert(escaped() == 1);

consteval std::meta::info leak(int &x) {
  int &r = x;
  return ^^r;
}
consteval int returned() {
  int x = 1;
  return std::meta::extract<int>(leak(x));
}
static_assert(returned() == 1);

consteval int bad_type() {
  int x = 1;
  int &r = x;
  return std::meta::extract<long &>(^^r);
}
static_assert(bad_type() == 1);

consteval int bad_const() {
  int x = 1;
  const int &r = x;
  return std::meta::extract<int &>(^^r);
}
static_assert(bad_const() == 1);
