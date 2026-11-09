// `-ftrapv` above -O0 shares one trap block per operation and function
// (clang `EmitTrapCheck`), with no `nomerge`. spec/codegen.md
// "Sanitizers", WP-1032.
//@ expect: ir
//@ std: c++20
//@ args: -ftrapv -Wno-c11-extensions -O2 -Xclang -disable-llvm-passes
#include "Inputs/trapv.h"
