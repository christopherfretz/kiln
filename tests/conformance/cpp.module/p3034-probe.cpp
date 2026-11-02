// [cpp.module]/2 (P3034R1, a DR): the module name and partition of a
// module declaration shall not be object-like macros (they are not
// expanded). Function-like macro names not followed by `(` are fine.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ xfail(clang): clang 19 and 22 expand the macros and accept
//@ diag: 13:15 error "module name component 'MOD' cannot be an object-like macro"
//@ diag: 13:19 error "module name component 'PART' cannot be an object-like macro"
module;
#define MOD foo
#define PART bar
export module MOD:PART;
#define FN(x) x
export int x;
