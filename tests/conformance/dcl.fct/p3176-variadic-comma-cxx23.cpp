// [dcl.fct]/3 (C++26, P3176R1): before C++26 a variadic ellipsis without
// a preceding comma is not deprecated.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
void f(int...);
void named(int n...);
template <class T> void t(T...);
