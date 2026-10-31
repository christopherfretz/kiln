// [over.oper]/7 (P1169, P2589): static operator() and operator[].
//@ expect: compile-ok
// C++23 static operator() and operator[] ([over.oper]/7, P1169, P2589).
struct F {
  static constexpr int operator()(int x) { return x * 2; }
  static constexpr int operator[](int x, int y) { return x - y; }
};
static_assert(F()(21) == 42);
static_assert(F{}[5, 3] == 2);
constexpr F f;
static_assert(f(4) + f[1, 1] == 8);
