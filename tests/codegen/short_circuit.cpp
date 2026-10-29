// &&, ||, ! and the conditional operator, as values and conditions.
int side(int);
bool sc(int a, int b, int* p) {
  bool x = a && side(b);
  bool y = (p && *p) || side(a);
  int z = a ? side(1) : b ? side(2) : 3;
  if (!x || (y && z)) return true;
  return a > 0 && b > 0 && a + b > 10;
}
