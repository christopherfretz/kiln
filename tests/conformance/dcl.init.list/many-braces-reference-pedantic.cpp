// [dcl.init.list]/3.10, [dcl.init.ref]: a reference list-initialized from
// a nested braced list of a scalar is an extension (an error under
// -pedantic-errors, clang's ext_many_braces_around_init). Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20 -pedantic-errors
//@ diag: 10:9 error "too many braces around scalar initializer"
//@ diag: 11:14 error "too many braces around scalar initializer"
int i;
int &&j{{1}};
const int &k{{i}};
// Valid: one level of braces, and an aggregate.
int &&j1{1};
const int &k1{i};
struct A { int x; };
const A &a{1};
