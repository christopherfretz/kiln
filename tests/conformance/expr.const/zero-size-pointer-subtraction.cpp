// [expr.add]/5: subtracting pointers to zero-size arrays (a GNU extension)
// is not a constant expression in clang. Found by the clang negative-test
// run (WP-912).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 9:15 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 9:22 note "subtraction of pointers to type 'int[0]' of zero size"
constexpr int (*p1)[0] = 0, (*p2)[0] = 0;
constexpr int k = p2 - p1;
