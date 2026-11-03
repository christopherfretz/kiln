//@ expect: compile-ok
//@ oracle: gcc
//@ xfail(kiln): kiln rejects it; GCC accepts it, so the expectation stands
// fake-gcc: accept
int f() { return nope; }
