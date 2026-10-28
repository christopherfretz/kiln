//@ expect: ast
int seq(int a, int b) {
  int c = (a++, b);
  (a, b) = 7;
  return c, a;
}
