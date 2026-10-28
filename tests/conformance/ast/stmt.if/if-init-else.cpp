//@ expect: ast
int classify(int v) {
  if (int r = v % 3; r == 0)
    return 0;
  else if (v < 0)
    return -1;
  if (int w = v - 1)
    return w;
  return 1;
}
