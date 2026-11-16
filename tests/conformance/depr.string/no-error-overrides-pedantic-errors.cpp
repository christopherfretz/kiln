// [depr.string], [intro.compliance]: clang 22 lets `-Wno-error=<group>` keep
// an extension warning that -pedantic-errors alone makes an error a warning
// (`-Wno-error=write-strings -pedantic-errors`; clang 19 still errors).
// Found running GCC's C++ testsuite (WP-1467).
//@ args: -pedantic-errors -Wno-error=write-strings -Wno-error=zero-length-array
//@ requires: clang>=22
//@ strict-diags
//@ diag: 10:11 warning "ISO C++11 does not allow conversion from string literal to 'char *'"
//@ diag: 11:7 warning "zero size arrays are an extension"
char *s = "foo";
int a[0];
