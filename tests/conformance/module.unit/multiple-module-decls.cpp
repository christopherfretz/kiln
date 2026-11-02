// [module.unit]/1: a translation unit has at most one module-declaration
// (clang 22 wording, reported at the `module` keywords).
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 10:8 error "translation unit contains multiple module declarations"
//@ diag: 9:8 note "previous module declaration is here"
export module M;
export module N;
export int y;
