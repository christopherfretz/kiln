// [module.import]/5, [cpp.import]/4: a header unit imported from its
// module file (`two-step`: every compiler builds it first with
// `-fmodule-header=user` and reads it with `-fmodule-file=`) gives the
// importer the header's declarations and its macros, as the in-process
// build does (header-unit.cpp).
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ args: -Wno-experimental-header-units
//@ header-unit: "Inputs/hu.h"
//@ two-step
import "Inputs/hu.h";

#ifndef HU_VALUE
#error "macros of a header unit are visible at the import"
#endif

static_assert(HU_ADD(1, 2) == 3);
constexpr HuPoint p{HU_VALUE, 2};
static_assert(p.x + p.y == 42);
int twice(int x) { return hu_twice(x); }
