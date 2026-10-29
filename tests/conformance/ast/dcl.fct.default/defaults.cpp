// [dcl.fct.default]: default arguments become CXXDefaultArgExpr at calls.
int f(int a, int b = 2, long c = sizeof(int));
int g(double d = 1.5);
void h() {
  f(1);
  f(1, 3);
  f(1, 3, 4);
  g();
}
int f(int a, int b, long c) { return a + b + int(c); }
