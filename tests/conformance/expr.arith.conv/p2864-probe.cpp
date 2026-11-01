// [expr.arith.conv]/1.3 (C++26, P2864R2): the usual arithmetic conversions
// between two different enumeration types, or an enumeration and a
// floating-point type, are ill-formed. WP-185 feature probe.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:11 error "invalid arithmetic between different enumeration types"
//@ diag: 11:14 error "invalid arithmetic between enumeration type"
enum E { a };
enum F { b };
int x = a + b;
double y = a * 1.0;
