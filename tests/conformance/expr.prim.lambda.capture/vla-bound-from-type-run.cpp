//@ expect: run
//@ strict-diags
//@ args: -std=gnu++20
//@ skip(clang): clang 22 crashes on typeof of a variable length array in a lambda
//@ skip(gcc): GCC 16 does not capture a two-dimensional variable length array (sorry, unimplemented)
//@ stdout: 60 20 20 60 64 68 60 12 7 25 12 60 12 13 20 12
// [expr.prim.lambda.capture] with variable length arrays (GNU extension):
// the bounds a lambda captures are found from the variably modified TYPE a
// body uses, not from how the type is spelled: `typeof` / `__typeof__` of a
// variable length array, `sizeof(typeof(a))`, a typedef named through
// `typeof`, a cast to a pointer to a variable length array, a local
// declared with such a type, pointer arithmetic on it, a nested lambda, and
// a generic lambda instantiated twice all use the size the array was
// created with, even after the bound variable changes. A variable length
// array type-id written in the lambda evaluates its bound there. Found
// running GCC's C++ testsuite. The expected output is the arrays' sizes,
// worked out by hand; vla-bound-from-type-1d-run.cpp is the GCC-checked
// one-dimensional sibling.
extern "C" int printf(const char *, ...);

void f(int n, int m) {
  int a[n][m];
  typedef int T[n];
  void *p = a;
  int n0 = n, m0 = m;
  n = 100;
  m = 100;
  int r1 = [&] { return (int)sizeof(typeof(a)); }();
  int r2 = [&] { return (int)sizeof(__typeof__(a[0])); }();
  n = n0; m = m0;
  int r3 = [&] { auto q = (int (*)[m])a; return (int)sizeof(*q); }();
  n = 100; m = 100;
  int r4 = [&] { return [&] { return (int)sizeof(typeof(a)); }(); }();
  auto g = [&](auto x) { return (int)sizeof(a) + (int)sizeof(x); };
  int r5 = g(1), r6 = g(1.0);
  int r7 = [&] { typeof(a) *q = &a; return (int)sizeof(*q); }();
  int r8 = [&] { return (int)sizeof(typeof(T)); }();
  int r9 = [&] { typeof(a) b; b[1][2] = 7; return b[1][2]; }();
  int r10 = [&] {
    typeof(a) *q = (typeof(a) *)p;
    (*q)[1][2] = 5;
    return a[1][2] + (int)sizeof(**q);
  }();
  int r11 = [&] { T *t = (T *)p; return (int)sizeof(*t); }();
  int r12 = [&] { return (int)((char *)((typeof(a) *)p + 1) - (char *)p); }();
  int r13 = [p] { return (int)sizeof(*(T *)p); }();
  auto h = [&](auto x) { return (int)sizeof(T) + (int)sizeof(x); };
  int r14 = h('a'), r15 = h(8L);
  n = n0; m = m0;
  int r16 = [&] { return (int)sizeof(int[n]); }();
  printf("%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n", r1, r2, r3, r4,
         r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15, r16);
}

int main() { f(3, 5); }
