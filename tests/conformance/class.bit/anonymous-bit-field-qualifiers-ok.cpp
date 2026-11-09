// [class.bit]/1: named bit-fields may be cv-qualified; unnamed ones are
// valid when unqualified (WP-1015).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
typedef const int CI;
struct S1 { int : 3; unsigned : 0; int a : 8; const int b : 3 = 1; };
struct S2 { CI x : 2; volatile int y : 4; S2() : x(0) {} };
static_assert(sizeof(S1) > 0);
