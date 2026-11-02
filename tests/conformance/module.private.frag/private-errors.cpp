// [module.private.frag]/1: a private module fragment is not exported from
// (no export declarations in it) and appears at most once.
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 15:1 error "export declaration cannot be used in a private module fragment"
//@ diag: 13:1 note "private module fragment begins here"
//@ diag: 16:9 error "private module fragment redefined"
//@ diag: 13:1 note "previous definition is here"
export module M;
export int a();
module :private;
int a() { return 1; }
export int b();
module :private;
