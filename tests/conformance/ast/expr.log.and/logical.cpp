//@ expect: ast
bool logic(int a, int *p, double d) {
  bool x = a && p;
  bool y = !a || d;
  return x && (y || !p);
}
