//@ expect: ast
//@ ast-filter: two
int one() { return 1; }
int two(int a) {
  int twofold = a * 2;
  return twofold;
}
