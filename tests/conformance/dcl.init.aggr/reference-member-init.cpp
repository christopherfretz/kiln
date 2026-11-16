// [dcl.init.aggr]/5: a reference member that is initialized explicitly or
// by its default member initializer is fine in aggregate initialization.
//@ std: c++20
int x;
struct A { int& r; };
A a1 = {x};
A a2{.r = x};
struct E { int& r = x; int i; };
E e1 = {};
E e2{};
struct D : A { int j; };
D d1 = {{x}, 1};
struct F { const int& r; };
F f1 = {1};
