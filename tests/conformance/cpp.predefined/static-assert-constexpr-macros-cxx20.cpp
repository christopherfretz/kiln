// [cpp.predefined]/1, [tab:cpp.predefined.ft]: at C++20 `__cpp_constexpr`
// is P1330's 202002L (changing the active member of a union in constant
// evaluation; clang 19 still says P1331's 201907L), and
// `__cpp_static_assert` is P2741's 202306L in every mode where it is
// defined, since kiln accepts user-generated messages as a C++26
// extension, as clang 22 does. WP-647 (2), (3); WP-851.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
static_assert(__cpp_constexpr == 202002L);
static_assert(__cpp_static_assert == 202306L);
union U { int a; float b; };
constexpr int f() { U u; u.a = 1; u.b = 2.0f; u.a = 3; return u.a; }
static_assert(f() == 3);
