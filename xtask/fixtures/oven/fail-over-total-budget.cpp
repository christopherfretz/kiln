//@ expect: compile-ok
//@ oven: steps<=115
constexpr int sq(int x) { return x * x; }
static_assert(sq(3) == 9);
