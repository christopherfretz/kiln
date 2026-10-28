//@ expect: ast
int ops(int x, int *p, bool b) {
  int y = -x + +x;
  y = ~y;
  b = !b;
  p = &x;
  *p = y++;
  return y-- + *p + b;
}
