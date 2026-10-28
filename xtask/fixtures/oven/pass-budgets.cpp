//@ expect: compile-ok
//@ oven: steps<=150 bytes<=64 peak_bytes<=32 calls<=3 allocs<=0 evaluations<=2
//@ oven: 6:1 steps<=100 calls<=2
//@ oven: 7 steps<=50
constexpr int sq(int x) { return x * x; }
static_assert(sq(3) == 9);
constexpr int v = sq(4);
