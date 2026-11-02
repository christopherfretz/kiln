// [cpp.module]/2 (P3034R1): a module name component that is a function-like
// macro name is not expanded (no `(` follows) and is not an error.
//@ std: c++20
//@ strict-diags
module;
#define FN(x) x
export module FN.ok;
export int x;
