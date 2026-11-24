// [cpp.pre]: `#__include_macros` is how the predefines carry `-imacros`;
// written in a source file it is an error and includes nothing (clang
// pp_include_macros_out_of_predefines). WP-2822.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:2 error "the #__include_macros directive is only for internal use by -imacros"
#__include_macros "does-not-exist.h"
int x;
