// C89 3.5.2 / 3.3.2.2: implicit int (a declaration with no type
// specifier declares an int), implicit function declarations (a call to
// an undeclared identifier declares `extern int f();`), and K&R function
// definitions (an identifier list plus a declaration list).
//@ expect: run
//@ std: c89
//@ args: -w
//@ stdout: 7 12 3
int printf(const char *, ...);
static counter = 7;
twice(x) { return x * 2; }
long mix(a, b, c) char a; double b; { return a + (long)b + c; }
main() {
  printf("%d %d %ld\n", counter, twice(6), mix(1, 1.5, 1));
  return undeclared_later();
}
undeclared_later() { return 0; }
