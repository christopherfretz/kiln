// GCC `#pragma GCC diagnostic ignored "-Wpedantic"` silences every
// extension diagnostic, also one with a group of its own: a zero-length
// array is accepted under -pedantic-errors between push and pop (clang 19
// and GCC 16.1). Found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-ok
//@ args: -pedantic-errors
//@ strict-diags
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wpedantic"
struct Z {
  unsigned trailing[0];
};
#pragma GCC diagnostic pop
