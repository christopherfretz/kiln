// [expr.const]/10.9: a read of a volatile object through a glvalue that
// drops the qualifier (const_cast of a pointer or a reference) is not a
// core constant expression (WP-948 siblings of read-volatile-via-cast).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 13:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 13:19 note "read of volatile object 'cv' is not allowed in a constant expression"
//@ diag: 12:20 note "volatile object declared here"
//@ diag: 15:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 15:19 note "read of volatile object 'vc' is not allowed in a constant expression"
//@ diag: 14:24 note "volatile object declared here"
const volatile int cv = 1;
constexpr int a = *const_cast<const int *>(&cv);
volatile constexpr int vc = 2;
constexpr int b = const_cast<const int &>(vc) + 1;
