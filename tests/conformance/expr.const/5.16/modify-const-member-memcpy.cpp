// [expr.const]/5.16, [dcl.type.cv]/4: `memcpy` into an object that holds a
// const member modifies that const subobject, which a constant expression
// may not do (GCC rejects it; clang 22 accepts it, copying bytes without
// looking at the members); into an object without one it is fine. Found
// running GCC's C++ testsuite (WP-1468).
//@ std: c++20
//@ requires: clang>=23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:32 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 16:15 note "in call to 'copy()'"
struct H { const int h = 1; int k = 0; };
struct K { int k = 0; };
constexpr int copy() { H a, b; __builtin_memcpy(&a, &b, sizeof a); return 0; }
static_assert(copy() == 0);
constexpr int plain() { K a, b{2}; __builtin_memcpy(&a, &b, sizeof a); return a.k; }
static_assert(plain() == 2);
