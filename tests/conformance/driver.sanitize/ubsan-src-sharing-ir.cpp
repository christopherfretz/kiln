// UBSan check data names the source file through one private `.src`
// string per distinct file name, shared by every check site in it and
// with an equal string literal (`__FILE__` here), as clang's
// `GetAddrOfConstantCString` cache does; the header's sites share a
// second one. spec/codegen.md "Sanitizers", WP-855, WP-1032.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=signed-integer-overflow,shift
#include "Inputs/src-sharing.h"

const char *file() { return __FILE__; }
int add(int a, int b) { return a + b; }
int shl(int a, int b) { return a << b; }
int sub(int a, int b) { return a - b; }
int use(int a) { return header_mul(a, a) + add(a, 1); }
const char *again() { return __FILE__; }
