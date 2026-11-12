// [dcl.typedef]/2, [temp.res.general]/3, [temp.inst]/4: a header's
// block-scope alias of a dependent name that does not exist for an
// instantiation's arguments is diagnosed at that instantiation, in the
// header. With `--pch-diff`, the image's local aliases give the same
// diagnostics, byte for byte (WP-1314).
//@ expect: compile-fail
//@ diag: 10:9 error "no matching function for call to 'f'"
#include "Inputs/local_typedef.h"
int a = ltd::S<int>::h();
int b = ltd::f();
