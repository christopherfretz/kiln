// [dcl.attr.grammar]/4 (GNU extension): clang 22's `nvptx_kernel` takes no
// arguments, and its argument is parsed as an expression before the
// count error; clang 19 does not know the attribute (spec/parse.md §6.2
// "Argument table", WP-1883). Verified against clang 19 and 22.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:16 error "'nvptx_kernel' attribute takes no arguments"
//@ diag: 14:29 error "use of undeclared identifier 'undeclared_c'"
//@ diag: 15:3 error "'clang::nvptx_kernel' attribute takes no arguments"
//@ diag: 16:29 error "use of undeclared identifier 'kiln_u'"

__attribute__((nvptx_kernel(1))) void f2();
__attribute__((nvptx_kernel(undeclared_c))) void f3();
[[clang::nvptx_kernel(1)]] void f5();
__attribute__((nvptx_kernel(kiln_u + 1))) void f6();
