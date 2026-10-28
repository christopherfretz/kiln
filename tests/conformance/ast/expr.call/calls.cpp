//@ expect: ast
void none();
int one(int);
double two(double, long);
int calls(short s) {
  none();
  int r = one(s);
  return r + (int)two(s, 'x') + one(one(2));
}
