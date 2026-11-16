// [dcl.init.aggr]/5: an uninitialized reference member is also an error
// in an implicitly initialized array element or nested aggregate, with
// clang's "in implicit initialization of ..." note naming the element.
// Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:18 error "reference member of type 'int &' uninitialized"
//@ diag: 16:24 note "uninitialized reference member is here"
//@ diag: 17:18 note "in implicit initialization of array element 1 with omitted initializer"
//@ diag: 19:9 error "reference member of type 'int &' uninitialized"
//@ diag: 14:17 note "uninitialized reference member is here"
//@ diag: 18:14 note "in implicit initialization of field 'a' with omitted initializer"
struct A { int& r; };
int x;
struct B { int i; int& r; };
B b2[2] = {{1, x}};
struct C { A a; int i; };
C c1 = {};
