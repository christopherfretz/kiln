// [dcl.init.aggr]/5: an uninitialized reference member is also an error
// in an implicitly initialized array element or nested aggregate. Not
// strict: clang adds an "in implicit initialization of ..." note kiln
// does not print yet. Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
// (see above: not strict)
//@ diag: 15:18 error "reference member of type 'int &' uninitialized"
//@ diag: 14:24 note "uninitialized reference member is here"
//@ diag: 17:9 error "reference member of type 'int &' uninitialized"
//@ diag: 12:17 note "uninitialized reference member is here"
struct A { int& r; };
int x;
struct B { int i; int& r; };
B b2[2] = {{1, x}};
struct C { A a; int i; };
C c1 = {};
