// [cpp.predefined]/1, [tab:cpp.predefined.ft]: at C++17 `__cpp_static_assert`
// is P2741's 202306L, as clang 22 has it in every mode (clang 19 keeps the
// pre-P2741 value), and `__cpp_constexpr` is P0170R1's 201603L. WP-851; WP-933
// diffed every -std= against clang++-22.
//@ std: c++17
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
static_assert(__cpp_static_assert == 202306L, "P2741");
static_assert(__cpp_constexpr == 201603L, "P0170R1");
