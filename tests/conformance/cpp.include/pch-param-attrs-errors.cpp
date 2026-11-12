// [dcl.attr.grammar]/5, [class.temporary]/6: a header's
// `[[clang::lifetimebound]]` parameters make the program's dangling
// references diagnosed at the call. With `--pch-diff`, the image's
// parameter attributes give the same diagnostics, byte for byte
// (WP-1314).
//@ expect: compile-fail
//@ diag: 10:37 error "returning reference to local temporary object"
//@ diag: 11:38 warning "temporary bound to local reference 'r' will be destroyed at the end of the full-expression"
#include "Inputs/param_attrs.h"
const int &bad1() { return pa::pick(1, 0); }
int main() { const int &r = pa::pick(4, 0); return r; }
