// [expr.const]/12: a converted constant expression allows a user-defined
// conversion followed by integral promotions and conversions, never a
// floating-integral conversion. A class converting to `double` is no
// argument for an `int` template parameter. Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ std: c++11
//@ diag: 14:3 error "conversion from 'const D' to 'int' is not allowed in a converted constant expression"

struct D { constexpr operator double() const { return 1.0; } };
template <int N> struct S {};
constexpr D d{};

S<d> s;
