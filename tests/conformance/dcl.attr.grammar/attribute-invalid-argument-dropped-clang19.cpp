// [dcl.attr.grammar]/4 (GNU extension): an attribute whose argument is
// invalid (an undeclared name, an operator over one) is dropped with that
// error alone; one whose argument recovered (a failed call, a missing
// member) is kept and checked further, as clang does (spec/parse.md §6.2
// "Argument table", WP-1880). Verified against clang 19 and 22.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:21 error "use of undeclared identifier 'f1'"
//@ diag: 29:16 error "'leaf' attribute takes no arguments"
//@ diag: 30:21 error "use of undeclared identifier 'kiln_u'"
//@ diag: 31:23 error "use of undeclared identifier 'kiln_u'"
//@ diag: 32:21 error "no matching function for call to 'g'"
//@ diag: 27:6 note "candidate function not viable: requires 0 arguments, but 1 was provided"
//@ diag: 32:16 error "'leaf' attribute takes no arguments"
//@ diag: 34:25 error "no member named 'zz' in 'S'"
//@ diag: 34:16 error "'leaf' attribute takes no arguments"
//@ diag: 35:22 error "use of undeclared identifier 'kiln_u'"
//@ diag: 36:28 error "use of undeclared identifier 'kiln_u'"
//@ diag: 37:27 error "use of undeclared identifier 'f1'"
//@ diag: 37:16 error "'alloc_size' attribute requires parameter 1 to be an integer constant"
//@ diag: 38:24 error "use of undeclared identifier 'f1'"
//@ diag: 38:16 error "'nonnull' attribute requires parameter 1 to be an integer constant"
//@ diag: 39:40 error "use of undeclared identifier 'f1'"
//@ diag: 39:16 error "'warn_unused_result' attribute takes no more than 1 argument"

void g();
int h(int);
__attribute__((leaf(f1()))) void a1();
__attribute__((leaf(kiln_u + 1))) void a2();
__attribute__((leaf(h(kiln_u)))) void a3();
__attribute__((leaf(g(1)))) void a4();
struct S { int m; };
__attribute__((leaf(S().zz))) void a6();
__attribute__((leaf((kiln_u)))) void a7();
__attribute__((leaf(sizeof(kiln_u)))) void a8();
__attribute__((alloc_size(f1()))) void *a9(int);
__attribute__((nonnull(f1()))) void a10(int *);
__attribute__((warn_unused_result("x", f1()))) int a11();
