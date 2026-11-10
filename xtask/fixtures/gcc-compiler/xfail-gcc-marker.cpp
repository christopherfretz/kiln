//@ expect: compile-fail
//@ xfail(gcc): the fake GCC accepts it
// fake-gcc: accept
int f() { return nope; }
