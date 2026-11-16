// [dcl.init.aggr]/5: an element of reference type that is not explicitly
// initialized and has no default member initializer makes aggregate
// initialization ill-formed (clang `err_init_reference_member_uninitialized`),
// a base's member too. Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:9 error "reference member of type 'int &' uninitialized"
//@ diag: 16:17 note "uninitialized reference member is here"
//@ diag: 19:6 error "reference member of type 'int &' uninitialized"
//@ diag: 16:17 note "uninitialized reference member is here"
//@ diag: 21:10 error "reference member of type 'int &' uninitialized"
//@ diag: 20:24 note "uninitialized reference member is here"
//@ diag: 23:9 error "reference member of type 'int &' uninitialized"
//@ diag: 16:17 note "uninitialized reference member is here"
struct A { int& r; };
int x;
A a1 = {};
A a2{};
struct B { int i; int& r; };
B b1 = {1};
struct D : A { int j; };
D d1 = {};
