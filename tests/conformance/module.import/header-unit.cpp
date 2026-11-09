// [module.import]/5, [cpp.import]/4: `import "header";` imports a header
// unit: the header's declarations are attached to the global module and
// the macros it defines are visible after the import. kiln builds the
// header unit in-process at the import; a clang builds it first with
// `-fmodule-header=user` (spec/harness.md `header-unit:`).
//@ expect: run
//@ std: c++20
//@ args: -Wno-experimental-header-units
//@ header-unit: "Inputs/hu.h"
import "Inputs/hu.h";

#ifndef HU_VALUE
#error "macros of a header unit are visible at the import"
#endif

static_assert(HU_ADD(1, 2) == 3);

int main() {
  HuPoint p{HU_VALUE, 2};
  return hu_twice(p.x) + p.y == 82 ? 0 : 1;
}
