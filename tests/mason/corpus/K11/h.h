inline int f(int a) {
  auto l = [a] { return a + VAL; };
  return l();
}
