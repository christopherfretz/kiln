// Its report breaches the `oven:` budget, which fails the bench entry.
//@ std: c++20
//@ oven: 6 steps<=10
constexpr int sq(int x) { return x * x; }
static_assert(sq(3) == 9);
constexpr int v = sq(4);
