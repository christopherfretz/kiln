// [cpp.module], [cpp.pre]/1 (P3868R1): a `#pragma` may precede the module
// declaration, and a `#pragma clang attribute` region open there covers
// the purview's declarations; clang 22 compiles this interface unit and
// marks `g` cold.
//@ expect: ir
//@ requires: clang>=22
//@ std: c++20
//@ args: -O0
#pragma clang attribute push (__attribute__((cold)), apply_to = function)
export module b;
export int g() { return 1; }
#pragma clang attribute pop
