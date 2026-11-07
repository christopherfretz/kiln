// [cpp.pre]/1 as amended by P3868R1 (a DR): only #line-like directives
// may precede the `module;` of a global module fragment. After a #define
// clang 22 rejects both the `module;` and the module declaration, with
// the note at the unit's first directive (clang 19 accepts).
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 15:1 error "'module;' introducing a global module fragment can appear only at the start of the translation unit"
//@ diag: 16:8 error "module declaration must occur at the start of the translation unit"
//@ diag: 13:1 note "add 'module;' to the start of the file to introduce a global module fragment"
// A comment does not count.
#pragma GCC diagnostic push
#define X 1
module;
export module M;
