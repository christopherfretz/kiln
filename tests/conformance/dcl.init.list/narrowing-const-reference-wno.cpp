// [dcl.init.list]/3.10: -Wno-error=c++11-narrowing-const-reference
// demotes the reference-member narrowing to a warning.
//@ verify
//@ args: -Wno-error=c++11-narrowing-const-reference

struct A { const int &r; };
double d;
A a1{d}; // expected-warning {{type 'double' cannot be narrowed to 'int' in initializer list}}
