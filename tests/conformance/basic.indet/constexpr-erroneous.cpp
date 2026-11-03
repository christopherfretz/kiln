// [expr.const] (C++26, P2795R5): an lvalue-to-rvalue conversion of an
// object with an erroneous or indeterminate value is not a core constant
// expression, whatever its type: the [basic.indet]/2 exemption for
// `unsigned char` and `std::byte` is a run-time rule, and a
// `[[indeterminate]]` variable is just as unreadable. The note is the one
// kiln and clang give for every uninitialized read (spec/oven.md).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:10 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 59:15 note "in call to 'plain()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:21 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 60:15 note "in call to 'uc()'"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:17 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 61:15 note "in call to 'byte()'"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:10 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 62:15 note "in call to 'indet()'"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:17 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 63:15 note "in call to 'element()'"
//@ diag: 69:13 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 69:13 note "subobject 'x' is not initialized"
//@ diag: 66:7 note "subobject declared here"
#include <cstddef>
constexpr int plain() {
  int x;
  return x;
}
constexpr int uc() {
  unsigned char c;
  unsigned char d = c;
  return d;
}
constexpr int byte() {
  std::byte b;
  std::byte e = b;
  return static_cast<int>(e);
}
constexpr int indet() {
  int x [[indeterminate]];
  return x;
}
constexpr int element() {
  int a[4];
  a[0] = 1;
  return a[0] + a[3];
}
constexpr int ok() {
  int x [[indeterminate]];
  x = 5;
  int y;
  y = x;
  return y;
}
static_assert(plain() == 0);
static_assert(uc() == 0);
static_assert(byte() == 0);
static_assert(indet() == 0);
static_assert(element() == 1);
static_assert(ok() == 5);
struct S {
  int x;
  constexpr S() {}
};
constexpr S s;
