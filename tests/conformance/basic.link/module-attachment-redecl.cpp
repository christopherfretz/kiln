// [basic.link]/11 (C++20 [module.unit]/7 attachment): redeclarations of an
// entity shall be attached to the same module; `extern "C++"` attaches a
// declaration in a module purview to the global module.
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 14:18 error "declaration of 'f' in the global module follows declaration in module M"
//@ diag: 13:5 note "previous declaration is here"
//@ diag: 16:5 error "declaration of 'g' in module M follows declaration in the global module"
//@ diag: 15:18 note "previous declaration is here"
export module M;
int f();
extern "C++" int f();
extern "C++" int g();
int g();
extern "C++" int h();
extern "C++" int h();
int k();
int k();
