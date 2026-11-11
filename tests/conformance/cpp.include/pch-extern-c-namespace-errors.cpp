// [dcl.link]/6-7: a variable in global scope conflicts with a C-linkage
// variable of the same name declared in a namespace of an included
// header, as without a precompiled header (WP-1310).
//@ expect: compile-fail
//@ diag: 8:5 error "declaration of 'ecn_var' in global scope conflicts with declaration with C language linkage"
#include "Inputs/extern_c_namespace.h"

int ecn_var = 10;
