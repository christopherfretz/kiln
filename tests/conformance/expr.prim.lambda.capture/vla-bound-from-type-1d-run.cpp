//@ expect: run
//@ strict-diags
//@ args: -std=gnu++20
//@ skip(clang): clang 22 crashes on typeof of a variable length array in a lambda
//@ oracle: gcc
//@ stdout: 12 12 12 7 12
// [expr.prim.lambda.capture] with a one-dimensional variable length array
// (GNU extension): the bound a lambda captures is found from the variably
// modified type the body uses (`typeof`, `__typeof__`, a local of that
// type, pointer arithmetic on a cast to a pointer to the array, a nested
// lambda), so the size is the one the array was created with.
// vla-bound-from-type-run.cpp covers the forms GCC cannot compile (two
// dimensions, typedefs, generic lambdas). Found running GCC's C++
// testsuite.
extern "C" int printf(const char *, ...);

void f(int n) {
  int a[n];
  void *p = a;
  n = 100;
  int r1 = [&] { return (int)sizeof(typeof(a)); }();
  int r2 = [&] { return (int)sizeof(__typeof__(a)); }();
  int r3 = [&] { return [&] { return (int)sizeof(typeof(a)); }(); }();
  int r4 = [&] { typeof(a) b; b[2] = 7; return b[2]; }();
  int r5 = [&] { return (int)((char *)((typeof(a) *)p + 1) - (char *)p); }();
  printf("%d %d %d %d %d\n", r1, r2, r3, r4, r5);
}

int main() { f(3); }
