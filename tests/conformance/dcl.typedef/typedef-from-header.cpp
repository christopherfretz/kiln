// [dcl.typedef]/4: a typedef redeclared in another file must name the same
// type; the previous definition may be in an included header or produced
// by a macro defined there. A diagnosed redefinition does not become the
// previous declaration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:13 error "typedef redefinition with different types ('int' vs 'unsigned long')"
//@ diag: 13:1 note "previous definition is here"
//@ diag: 15:1 error "typedef redefinition with different types ('long' vs 'unsigned long')"
//@ diag: 13:1 note "previous definition is here"
#include "Inputs/size.h"
typedef unsigned long Size;
MAKE_SIZE(unsigned long)
typedef int Size;
MAKE_SIZE(long)
