inline int f(int x) {
  auto l = [CAP] { return x; };
  return l();
}
