//@ expect: ast
//@ ast-filter: f
// [dcl.attr.assume]: an assumption is an `AttributedStmt` over the null
// statement carrying `CXXAssumeAttr` with the converted assumption as its
// child (contextual bool conversion, full-expression cleanups); a list of
// two attributes is one statement with two attributes. In a template
// pattern the assumption stays dependent and unconverted. Matches clang 19.
int g(int);
int f(int x, int *p) {
  [[assume(x > 0)]];
  [[assume(p), assume(x != 3)]];
  [[assume(g(x))]];
  return x;
}
template <class T> T f(T v) {
  [[assume(v > T(0))]];
  return v;
}
