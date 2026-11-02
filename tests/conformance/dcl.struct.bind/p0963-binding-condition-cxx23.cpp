// [stmt.pre]/4 (C++26, P0963R3): before C++26 a structured binding
// declaration as a condition is accepted as an extension, with a warning
// for each (clang 19 -Wbinding-in-condition, "ISO C++17 does not
// permit ..."; clang 22 -Wc++26-extensions, "... is a C++2c extension").
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ diag: 16:12 warning "structured binding declaration in a condition"
//@ diag: 17:15 warning "structured binding declaration in a condition"
//@ diag: 18:15 warning "structured binding declaration in a condition"
struct P {
  int x, y;
  explicit operator bool() const { return x != 0; }
};
int f(P p) {
  if (auto [a, b] = p) return a;
  while (auto [c, d] = p) return d;
  for (; auto [e, g] = p;) return e;
  return 0;
}
