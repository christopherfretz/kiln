inline int f(int x) {
  auto l = [=] { return x; };
  return l();
}
int use_a() { return f(1); }
