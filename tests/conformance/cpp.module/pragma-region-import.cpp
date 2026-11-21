// [cpp.import], [module.import]/1: an import inside a `#pragma clang
// attribute` region is an import like any other (clang 22 accepts it; the
// pragma applies to the declarations after it).
//@ std: c++20
//@ strict-diags
//@ module: m=Inputs/pragma-m.cppm
#pragma clang attribute push (__attribute__((cold)), apply_to = function)
import m;
int g() { return pragma_m(); }
#pragma clang attribute pop
