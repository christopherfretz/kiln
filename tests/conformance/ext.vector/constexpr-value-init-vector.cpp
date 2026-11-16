// A value-initialized GNU vector (`V4()`) is all zero, also in a constant
// initializer. Found running GCC's C++ testsuite (WP-1468).
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-ok
typedef float V4 __attribute__((vector_size(16)));
constexpr V4 v = V4() + 1.0f;
static_assert(v[0] == 1.0f && v[3] == 1.0f);
constexpr V4 z = V4();
static_assert(z[2] == 0.0f);
