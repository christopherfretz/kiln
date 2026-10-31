//@ expect: ast
//@ oracle: clang
// [over.match.oper]/2 (spec/sema-overload.md §6): member and non-member
// operator functions build CXXOperatorCallExpr; a prvalue object of a
// member call is materialized; built-in candidates convert class operands.
struct V {
  int x;
  V operator+(const V &) const;
  V &operator+=(int);
  int operator[](int) const;
  int operator()(int, int) const;
  bool operator!() const;
};
V operator-(V);
struct I { operator int() const; };
int f(V a, V b, I i) {
  V c = a + b;
  c += 1;
  V d = -c;
  (void)!V{};
  return a[0] + a(1, 2) + (i + 1) + V{}[1];
}
