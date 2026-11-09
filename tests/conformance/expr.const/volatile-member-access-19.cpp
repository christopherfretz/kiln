// [basic.types.general]/10: clang 19 (before P2448R2) treats a class with a
// volatile member as non-literal, so a local of that class already fails at
// its initializer. The clang 22 behaviour is volatile-member-access-clang22.cpp.
// WP-1019.
//@ std: c++26
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:15 error "constexpr variable 'k1' must be initialized by a constant expression"
//@ diag: 16:25 note "non-literal type 'S' cannot be used in a constant expression"
//@ diag: 17:20 note "in call to 'm1()'"
//@ diag: 20:15 error "constexpr variable 'k2' must be initialized by a constant expression"
//@ diag: 19:25 note "non-literal type 'N' cannot be used in a constant expression"
//@ diag: 20:20 note "in call to 'm2()'"
struct S { int a; volatile int v; };
constexpr int m1() { S s{1, 2}; return s.a; }
constexpr int k1 = m1();
struct N { S s; };
constexpr int m2() { N n{{1, 2}}; return n.s.a; }
constexpr int k2 = m2();
