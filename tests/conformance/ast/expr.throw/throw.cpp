//@ expect: ast
void f(int x, const int cx, int *p, int &r) {
  throw;
  throw 1;
  throw x;
  throw cx;
  throw r;
  throw "abc";
  throw f;
  throw p;
  throw (x);
  int v = x ? throw 1 : 2;
  x ? throw 1 : throw 2;
  static int s;
  throw s;
}
constexpr int g(int x) { return x > 0 ? x : throw 0; }
constexpr int h = g(1);
