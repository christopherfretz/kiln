// [module.import]/1, [module.unit]: in a module unit, imports must come
// before any other declaration of the purview (clang 22 wording).
//@ expect: compile-fail
//@ std: c++20
//@ module: a=Inputs/a.cppm
//
//@ strict-diags
//@ diag: 11:1 error "imports must immediately follow the module declaration"
export module M;
export int x;
import a;
