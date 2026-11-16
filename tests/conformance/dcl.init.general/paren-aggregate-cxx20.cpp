// [dcl.init.general]/16.6.2.2 (C++20, P0960): an aggregate or an array is
// initialized from a parenthesized expression list.
//@ std: c++20
struct P { int a, b; };
P p1(1, 2);
P p2(1);
int arr[2](1, 2);
P p3(p1);
static_assert(P(5, 6).b == 6, "");
