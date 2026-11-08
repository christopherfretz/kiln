// spec/reflection.md §2.5 (decided): a byte-wise copy of an info object is
// a typed copy, so memcpy/memmove between an info and an object of another
// type is not a constant expression (WP-830 pins it; GCC 16.1 rejects too,
// as it does not constant-evaluate memcpy at all).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ oracle: gcc
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:3 note "cannot constant evaluate 'memcpy' from object of type 'std::meta::info' to object of type 'unsigned long'"
//@ diag: 28:15 note "in call to 'to_bytes()'"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:3 note "cannot constant evaluate 'memmove' from object of type 'unsigned long' to object of type 'std::meta::info'"
//@ diag: 36:15 note "in call to 'from_bytes()'"
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif

consteval bool to_bytes() {
  std::meta::info a = ^^int;
  unsigned long b = 0;
  __builtin_memcpy(&b, &a, sizeof b);
  return b != 0;
}
static_assert(to_bytes());

consteval bool from_bytes() {
  unsigned long b = 1;
  std::meta::info a;
  __builtin_memmove(&a, &b, sizeof b);
  return a == ^^int;
}
static_assert(from_bytes());
