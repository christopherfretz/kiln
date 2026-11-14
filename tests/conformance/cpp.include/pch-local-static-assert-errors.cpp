// [dcl.pre]/10-11, [temp.inst]/4: a header's block-scope static assertion
// that fails for an instantiation's arguments is diagnosed at that
// instantiation, in the header, with the point of instantiation in the
// unit. With `--pch-diff`, the snapshot's local assertions give the same
// diagnostics, byte for byte (WP-1314).
//@ expect: compile-fail
//@ diag: 13:9 error "no matching function for call to 'pick'"
#include "Inputs/local_static_assert.h"
struct Big { char c[100]; operator int() const { return 0; } };
struct Three { char c[3]; };
int a = lsa::f(Big{});
int b = lsa::S<Three>::h();
int c = lsa::pick<int>(nullptr);
