// GCC `#pragma GCC diagnostic ignored "-Wpedantic"` silences every
// extension diagnostic, also one with a group of its own: a zero-length
// array is accepted under -pedantic-errors between push and pop (clang 19
// and GCC 16.1). Found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-ok
//@ args: -pedantic-errors
//@ strict-diags
//@ xfail(kiln): WP-1495, the pedantic group holds only group-less extensions, so ignoring -Wpedantic leaves -Wzero-length-array an error
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wpedantic"
struct Z {
  unsigned trailing[0];
};
#pragma GCC diagnostic pop
