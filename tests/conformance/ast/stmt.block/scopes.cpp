//@ expect: ast
int shadow(int x) {
  int y = x;
  {
    int x = 2;
    y += x;
    {
      int y = 5;
      x += y;
    }
  }
  ;
  return y;
}
