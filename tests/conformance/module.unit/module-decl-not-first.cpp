// [module.unit]/1, [module.global.frag]/1: a module declaration must be
// the first declaration of a module unit unless a global module fragment
// (`module;`) introduces the declarations before it (clang 22 wording,
// reported at the `module` keyword).
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 12:8 error "module declaration must occur at the start of the translation unit"
//@ diag: 11:1 note "add 'module;' to the start of the file to introduce a global module fragment"
int x;
export module M;
export int y;
