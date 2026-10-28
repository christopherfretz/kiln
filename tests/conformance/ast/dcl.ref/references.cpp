//@ expect: ast
int g;
int &r = g;
const int &cr = g;
int &pick(int &a, int &b, bool c) { return c ? a : b; }
int use(int v) {
  int &q = pick(r, v, true);
  q = 3;
  return cr + q;
}
