// [stmt.label], [dcl.attr.annotation]/1: an attribute-specifier-seq after a
// label's colon appertains to the labeled declaration statement's
// entities, so an annotation there is the variable's (WP-830).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
void h() {
lbl:
  [[=4]] int x = 0;
  (void)x;
  static_assert(std::meta::annotations_of(^^x).size() == 1);
  static_assert(std::meta::extract<int>(std::meta::annotations_of(^^x)[0]) == 4);
}
