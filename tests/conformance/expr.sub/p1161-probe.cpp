// [depr.comma.subscript], [expr.sub]/2 (C++20, P1161R3): a top-level comma
// expression in a subscript is still a comma expression in C++20, and
// deprecated. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ xfail(kiln): kiln parses `a[i, j]` as a C++23 multidimensional subscript at -std=c++20 ("type 'int[3]' does not provide a subscript operator") instead of a deprecated comma expression
//@ diag: 11:35 warning "top-level comma expression in array subscript is deprecated in C++20 and unsupported in C++23"
int a[3];
int g(int i, int j) { return a[(i++, j)]; }
int f(int i, int j) { return a[i++, j]; }
