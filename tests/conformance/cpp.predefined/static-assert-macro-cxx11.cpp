// [cpp.predefined]/1, [tab:cpp.predefined.ft]: `__cpp_static_assert` is
// P2741's 202306L from C++11 on, as clang 22 has it (clang 19: N1720's
// 200410), and C++11's `__cpp_constexpr` is untouched. WP-647 (2); WP-851.
//@ std: c++11
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
static_assert(__cpp_static_assert == 202306L, "P2741");
static_assert(__cpp_constexpr == 200704, "N2235");
