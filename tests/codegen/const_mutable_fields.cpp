// WP-183: a `const` object of a class type with a `mutable` member, directly
// or in a member or array element, is emitted `global`, not `constant`
// (clang `isTypeConstant`: `hasMutableFields`) -- namespace-scope, static
// member, static local and `constexpr` alike. A lax fold
// ([expr.const]/5.9, clang `EvaluateAsRValue`) reads its non-mutable
// members (`mm.k` folds to 7) but not its mutable ones (`mm.m` is loaded).
struct S { int k; mutable int m; };
struct In { S s; };
const S mm{7, 8};
const In in{{1, 2}};
const S arr[2] = {{1, 2}, {3, 4}};
constexpr S cs{9, 10};
struct H { static const S hs; };
const S H::hs{11, 12};
int f() { return mm.k + mm.m; }
int g() { return in.s.k + arr[1].k + cs.k + cs.m; }
int h() {
  static const S ls{5, 6};
  return ls.k + ls.m + H::hs.k;
}
const S *use() { return &in.s; }
const S *use3() { return arr; }
