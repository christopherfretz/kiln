// C23 6.7.11p4, 6.6p9 (GNU labels as values): a block-scope static
// initialized with differences of label addresses (`&&a - &&b`) is a
// constant initializer in GNU C; it is initialized before program startup
// and needs neither a guard variable nor the C++ runtime, so a jump past
// its declaration (6.8.7.2p1) still sees the value.
//@ args: -w
//@ expect: run
//@ xfail(kiln): WP-1301 follow-up, a label-address difference in a static initializer does not fold, so the static is initialized dynamically under a __cxa_guard that a C link cannot resolve and a jump skips (gcc.c-torture execute/pr70460.c)
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
