// `-fsanitize=undefined,implicit-conversion` (less the unimplemented
// function and vptr checks) at -O2: the type, bool/enum range and
// conversion checks combined on one access, each in clang's order.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=undefined,implicit-conversion -fno-sanitize=function,vptr -O2 -Xclang -disable-llvm-passes
#include "Inputs/ub-typecheck.h"
#include "Inputs/ub-boolenum.h"
