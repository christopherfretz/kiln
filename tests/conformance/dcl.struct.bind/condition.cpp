//@ expect: ast
//@ ast-filter: test
// [stmt.pre]/4 (C++26 P0963): a structured binding declaration as a
// condition. clang 19 accepts it as an extension with
// -Wbinding-in-condition, testing the holding variable (here through its
// explicit operator bool); an init-statement is plain C++17.
//@ diag: 9:12 warning "ISO C++17 does not permit structured binding declaration in a condition"
//@ diag: 10:15 warning "ISO C++17 does not permit structured binding declaration in a condition"
struct P { int x, y; explicit operator bool() const { return x; } };
int test_cond(P p) {
  if (auto [a, b] = p) return a;
  while (auto [c, d] = p) return d;
  if (auto [e, g] = p; e) return g;
  switch (auto [h, i] = p; h) { default: return i; }
}
