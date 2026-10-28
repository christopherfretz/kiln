//@ expect: ast
int counts(int *p, double d, char c) {
  int i = 0;
  i++;
  ++i;
  i--;
  --i;
  p++;
  --p;
  d++;
  c--;
  return i + *p++ + *++p;
}
