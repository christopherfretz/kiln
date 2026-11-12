// [class.compare.default]/6, [temp.inst]/4: a header's class template's
// defaulted `operator==` is deleted for an instantiation whose member has
// no usable `operator==`, diagnosed at the use with notes in the header.
// With `--pch-diff`, the image's defaulted comparisons give the same
// diagnostics, byte for byte (WP-1314). The member form is used: for the
// friend form (`dc::box`) clang words the error as "overload resolution
// selected deleted operator '=='" and kiln does not (a wording note).
//@ expect: compile-fail
//@ diag: 12:24 error "object of type 'dc::mbox<Z>' cannot be compared because its 'operator==' is implicitly deleted"
#include "Inputs/defaulted_cmp.h"
struct Z {};
bool f = dc::mbox<Z>{} == dc::mbox<Z>{};
