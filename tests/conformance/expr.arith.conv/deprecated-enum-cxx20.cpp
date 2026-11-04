// [depr.arith.conv.enum] (C++20, C++23): the usual arithmetic conversions
// between two different enumeration types, or an enumeration and a
// floating-point type, are deprecated, not ill-formed (that is C++26,
// P2864): clang's -Wdeprecated-*-enum-* warnings, at the operator.
//@ std: c++23
//@ strict-diags
//@ diag: 13:28 warning "arithmetic between different enumeration types ('Q' and '(unnamed enum at"
//@ diag: 14:28 warning "arithmetic between different enumeration types ('Q' and 'R') is deprecated"
//@ diag: 15:28 warning "arithmetic between floating-point type 'double' and enumeration type 'Q' is deprecated"
//@ diag: 16:27 warning "arithmetic between enumeration type 'Q' and floating-point type 'double' is deprecated"
//@ diag: 17:29 warning "comparison of different enumeration types ('Q' and 'R') is deprecated"
enum Q { A }; enum R { B }; enum { N = 4 };
int f(Q q, R r) { return q % N; }
int g(Q q, R r) { return q + r; }
double h(Q q) { return 1.0 / q; }
double h2(Q q) { return q * 1.0; }
bool k(Q q, R r) { return q == r; }
