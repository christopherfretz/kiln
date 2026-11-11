// C17 6.9.1p12: flowing off the end of a non-void function is undefined
// only if the caller uses the value, so C gets no trap there (clang emits
// C++'s llvm.trap/unreachable only in C++). Reaching the end of main
// returns 0 (5.1.2.2.3).
//@ std: c17
//@ expect: run
//@ stdout: 2
int printf(const char *, ...);
static int calls;
int count(int x) {
  calls += x;
  if (x > 5)
    return x;
}
int main(void) {
  count(1);
  count(1);
  count(0);
  printf("%d\n", calls);
}
