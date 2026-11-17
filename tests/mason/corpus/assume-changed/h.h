inline int f(int a) {
  [[assume(a > N)]];
  return a / 2;
}
