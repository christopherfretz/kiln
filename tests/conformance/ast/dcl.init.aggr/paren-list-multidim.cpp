//@ expect: ast
//@ ast-filter: f
// [dcl.init.general]/16.5, /16.6.2.2, /9 (P0960; WP-182): in a
// parenthesized initialization of a multidimensional array, a
// braced-init-list argument copy-list-initializes its row, and the
// value-initialized rows of class type are one CXXConstructExpr of the row
// type (zeroing unless the default constructor is user-provided); a
// value-initialized member that is an array of classes is the same.
// Matches clang 19.
struct S { S(); S(int); int v; };
struct T { int v; };
struct D { int x; S s[2]; };
void f(int n) {
  S a[2][3]({S(1)});
  T b[2][2]({T{1}});
  int c[2][2]({1, 2});
  D d(1);
  (void)new S[n][2]({S(4)});
}
