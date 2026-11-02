// [module.global.frag]/1, [cpp.module]: `module;` introducing a global
// module fragment must start the translation unit.
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 9:1 error "'module;' introducing a global module fragment can appear only at the start of the translation unit"
export module M;
module;
export int y;
