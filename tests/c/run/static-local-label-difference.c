// C23 6.7.11p4, 6.6p9 (GNU labels as values): a block-scope static
// initialized with differences of label addresses (`&&a - &&b`) is a
// constant initializer in GNU C; it is initialized before program startup
// and needs neither a guard variable nor the C++ runtime, so a jump past
// its declaration (6.8.7.2p1) still sees the value.
//@ args: -w
//@ expect: run
//@ stdout: 3 4
int printf(const char *, ...);
int c;
void foo(int x) {
  goto start;
  static int b[] = {&&lab1 - &&lab0, &&lab2 - &&lab0};
start:
  goto *(&&lab0 + b[x]);
lab1:
  c += 2;
lab2:
  c++;
lab0:;
}
int main(void) {
  foo(0);
  int first = c;
  foo(1);
  printf("%d %d\n", first, c);
  return 0;
}
