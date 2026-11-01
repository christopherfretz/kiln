//@ expect: run
// [dcl.attr.likelihood], [dcl.attr.fallthrough] (WP-182): attributed case
// labels and fallthrough statements at run time.
int f(int x) {
  int r = 0;
  switch (x) {
  [[likely]] case 1: r += 1; [[fallthrough]];
  [[unlikely]] case 2: r += 10; break;
  case 3: [[fallthrough]];
  [[likely]] default: r += 100;
  }
  if (r > 5) [[likely]] { r *= 2; } else [[unlikely]] r -= 1;
  [[likely]] return r;
}
int main() {
  volatile int a = 1, b = 2, c = 3, d = 9;
  return !(f(a) == 22 && f(b) == 20 && f(c) == 200 && f(d) == 200);
}
