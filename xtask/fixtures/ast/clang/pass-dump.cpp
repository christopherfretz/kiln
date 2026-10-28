//@ expect: ast
constexpr int sq(int x) { return x * x; }
constexpr int k = sq(3);
