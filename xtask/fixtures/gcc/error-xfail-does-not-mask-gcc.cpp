//@ expect: compile-ok
//@ oracle: gcc
//@ xfail(kiln): kiln rejects it; GCC's verdict is still checked
// fake-gcc: reject
int f() { return nope; }
