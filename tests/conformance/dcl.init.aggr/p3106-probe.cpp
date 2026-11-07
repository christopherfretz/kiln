// P3106R1 ([dcl.init.aggr], a DR): brace elision in an initializer list
// of an aggregate with array members elides consistently.
//@ expect: compile-ok
//@ std: c++26
//@ strict-diags
//@ args: -Wno-missing-braces
struct S1 { int a[2]; int b; };
constexpr S1 s1 = {1, 2, 3};
static_assert(s1.a[1] == 2 && s1.b == 3);
struct X { int a, b; };
struct Y { X x[2]; };
constexpr Y y = {1, 2, 3, 4};
static_assert(y.x[1].a == 3 && y.x[1].b == 4);
constexpr int arr[2][2] = {1, 2, 3, 4};
static_assert(arr[1][0] == 3);
