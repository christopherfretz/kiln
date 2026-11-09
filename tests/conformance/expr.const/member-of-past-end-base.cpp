// [expr.const]/10.8: pb1 points past the end of the base subobject; reading
// pb1->n is undefined. Found by the clang negative-test run (WP-912).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 12:15 error "constexpr variable 'n' must be initialized by a constant expression"
//@ diag: 12:24 note "cannot access field of pointer past the end of object"
struct Base { int n; };
struct Derived : Base {};
constexpr Derived a[2] = {};
constexpr const Base *pb = &a[0];
constexpr const Base *pb1 = pb + 1;
constexpr int n = pb1->n;
