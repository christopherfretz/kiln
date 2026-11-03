// spec/reflection-values.md §6, WP-479 (D5): the reflection bytes of 10^5
// distinct reflect_constant values (about 4.5 MB, reflect-constant-1e5.cpp)
// count against -fconstexpr-memory, so a 64 KiB limit stops the
// evaluation at the call that crosses it, with the existing memory-limit
// error, instead of growing the translation unit's tables unboundedly.
//@ args: -freflection -I tests/support -fconstexpr-memory=65536
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:9 note "constexpr evaluation exceeded the memory limit of 65536 bytes"
//@ diag: 21:15 note "in call to 'run(100000)'"
//@ mem<=256
#include "kiln_meta.h"
consteval int run(int n) {
  int hits = 0;
  for (int i = 0; i < n; ++i)
    if (std::meta::reflect_constant(i) != std::meta::info()) ++hits;
  return hits;
}
static_assert(run(100000) == 100000);
