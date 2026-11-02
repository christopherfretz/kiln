// [module.import]/5, [cpp.import]/4: `import "header";` imports a header
// unit: the header's declarations are attached to the global module and
// the macros it defines are visible after the import.
//@ expect: run
//@ std: c++20
//@ skip(clang): clang needs -fmodule-header to build Inputs/hu.h as a header unit first, which the harness cannot drive
import "Inputs/hu.h";

#ifndef HU_VALUE
#error "macros of a header unit are visible at the import"
#endif

static_assert(HU_ADD(1, 2) == 3);

int main() {
  HuPoint p{HU_VALUE, 2};
  return hu_twice(p.x) + p.y == 82 ? 0 : 1;
}
