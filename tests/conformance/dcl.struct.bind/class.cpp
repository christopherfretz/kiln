//@ expect: ast
//@ ast-filter: test
// [dcl.struct.bind]/5 (C++26 /7): binding the non-static data members of a
// class, in declaration order. All of them must be direct members of the
// same class (here a base, B, holds them all); a mutable member stays
// non-const through `const auto`, and the bindings are lvalues naming
// `e.m` (MemberExpr), through a derived-to-base cast when inherited.
struct B { int x; long y; };
struct D : B {};
struct M { int i; mutable int m; };
int test_base(D d) {
  auto [x, y] = d;
  return x + int(y);
}
int test_mutable(M mm) {
  const auto [i, m] = mm;
  m = 3;
  return i + m;
}
int test_temporary() {
  const auto &[i, m] = M{1, 2};
  auto &&[j, n] = M{3, 4};
  return i + m + j + n;
}
