// [dcl.constexpr]/6: clang 22 constant-evaluates `__builtin_abs` (its
// `Constexpr` marker, clang 20), so a call of it with unknown operands is
// possibly constant (WP-945; the clang 19 shape is
// never-constant-builtin-abs-clang19.cpp). spec/sema.md "Never constant".
//@ requires: clang>=22
//@ std: c++20
//@ strict-diags
//@ expect: compile-ok
constexpr int a1(int x) { return __builtin_abs(-1) + x; }
constexpr int a2(int x) { return __builtin_abs(x); }
