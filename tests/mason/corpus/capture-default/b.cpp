inline int f(int x) {
  auto l = [x] { return x; };
  return l();
}
int use_b() { return f(1); }
