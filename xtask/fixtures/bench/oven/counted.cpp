// Constexpr-counter entry for `cargo xtask bench`, compiled by the fake kiln
// (`xtask/fixtures/fake-kiln/kiln`), which reports `counted.cpp.oven.jsonl`.
//@ std: c++20
//@ oven: steps<=200
//@ threshold-bytes: 50
constexpr int sq(int x) { return x * x; }
static_assert(sq(3) == 9);
constexpr int v = sq(4);
