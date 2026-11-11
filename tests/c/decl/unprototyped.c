// C17 6.7.6.3p14, 6.5.2.2p6: before C23, `int f()` declares a function
// without a prototype; a call through it applies the default argument
// promotions (float to double, char/short to int). `f(void)` takes no
// arguments.
//@ expect: run
//@ std: c17
//@ args: -Wno-deprecated-non-prototype
//@ stdout: 3.500000 97 6
int printf(const char *, ...);
double half();
int ch();
int sum();
int main(void) {
  float f = 7.0f;
  char c = 'a';
  printf("%f %d %d\n", half(f), ch(c), sum(1, 2, 3));
  return 0;
}
double half(double d) { return d / 2; }
int ch(int c) { return c; }
int sum(a, b, c) int a, b, c; { return a + b + c; }
