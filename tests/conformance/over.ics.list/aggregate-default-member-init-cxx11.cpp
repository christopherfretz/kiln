// C++11 [dcl.init.aggr]/1: a class with a default member initializer is not
// an aggregate (N3653 changed this in C++14), so a braced list does not
// convert to it by aggregate initialization.
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 18:12 error "no matching function for call to 'take'"
//@ diag: 17:6 note "candidate function not viable: cannot convert initializer list argument to 'A'"
//@ diag: 22:3 error "no matching constructor for initialization of 'B'"
//@ diag: 19:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const B' for 1st argument"
//@ diag: 19:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'B' for 1st argument"
//@ diag: 19:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"

struct A {
  char str[4] = "foo";
};
void take(A);
void g() { take({"abc"}); }
struct B {
  int x = 1;
};
B b = {2};
