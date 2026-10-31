//@ expect: ast
//@ oracle: clang
// [over.oper]/7 (P1169, P2589): static operator() and operator[]; the
// object expression is still evaluated.
struct F {
  static int operator()(int);
  static int operator[](int, int);
};
int f(F g) { return g(1) + F{}(2) + g[1, 2]; }
