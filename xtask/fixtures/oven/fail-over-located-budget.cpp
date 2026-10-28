//@ expect: compile-ok
//@ oven: 4:15 calls<=0
constexpr int sq(int x) { return x * x; }
constexpr int v = sq(4);
