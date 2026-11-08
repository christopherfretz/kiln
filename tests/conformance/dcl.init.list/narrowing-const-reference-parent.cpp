// [dcl.init.list]/3.10: -Wc++11-narrowing-const-reference is a subgroup
// of -Wc++11-narrowing, so -Wno-c++11-narrowing turns it off.
//@ verify
//@ args: -Wno-c++11-narrowing
// expected-no-diagnostics

struct A { const int &r; };
double d;
A a1{d};
