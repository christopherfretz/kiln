// The evaluation at line 5 of some other file must not satisfy `oven: 5`.
//@ expect: compile-ok
//@ oven: 5 steps<=1000
constexpr int v = 4;
