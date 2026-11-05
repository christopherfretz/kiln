// [expr.arith.conv]/1.3 (C++26, P2864R2): arithmetic between two different
// enumeration types, or an enumeration and a floating-point type, is
// ill-formed (p2864-probe.cpp). GCC 16.1 makes it a pedwarn, a warning by default
// (-Wdeprecated-enum-enum-conversion), and code built with GCC and
// -fpermissive relies on it (BDE-era headers computing `queue % Num_Areas`
// across an unnamed enumeration): under -fpermissive kiln diagnoses it with
// the C++20/23 deprecation warning instead (spec/driver.md).
//@ std: c++26
//@ args: -fpermissive
//@ skip(clang): clang's -fpermissive does not downgrade P2864 enum arithmetic (GCC's does; the behaviour is kiln's)
//@ strict-diags
//@ diag: 17:28 warning "arithmetic between different enumeration types ('Q' and '(unnamed enum at"
//@ diag: 18:28 warning "arithmetic between different enumeration types ('Q' and 'R') is deprecated"
//@ diag: 19:26 warning "arithmetic between enumeration type 'Q' and floating-point type 'double' is deprecated"
//@ diag: 20:17 warning "arithmetic between different enumeration types ('Q' and 'R') is deprecated"
enum Q { A }; enum R { B }; enum { N = 4 };
int f(Q q, R r) { return q % N + 1; }
int g(Q q, R r) { return q + r; }
double h(Q q) { return q * 1.0; }
static_assert(A + B == 0);
