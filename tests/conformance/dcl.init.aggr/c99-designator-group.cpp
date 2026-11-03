// [dcl.init.aggr]/4 and the -W group tree: from clang 22 on,
// -Wc99-designator has -Winitializer-overrides and -Wreorder-init-list as
// subgroups, so -Wno-c99-designator silences both, also for C++20
// designated initializers; a later -Wreorder-init-list re-enables its own.
//@ expect: compile-ok
//@ args: -Wno-c99-designator -Wreorder-init-list
//@ requires: clang>=22
//@ strict-diags
//@ diag: 15:33 warning "ISO C++ requires field designators to be specified in declaration order; field 'z' will be initialized after field 'a'"
//@ diag: 15:25 note "previous initialization for field 'z' is here"
struct A { int x, y; };
struct B { A a; int z; };
A mk();
B b0 = {.a = mk(), .a = {1, 2}};
B b1 = {.a = mk(), .z = 4, .a = {1, 2}};
B b2 = {.a = mk(), .a.y = 3};
