// Accounted-memory entry for `cargo xtask bench` whose memory report has a
// malformed total (`bad-mem.cpp.mem`), printed by the fake kiln
// (`xtask/fixtures/fake-kiln/kiln`): the entry fails.
//@ std: c++20
constexpr int sq(int x) { return x * x; }
static_assert(sq(3) == 9);
