// [dcl.fct]/5: a parameter of type "array of T" is adjusted to "pointer
// to T" whatever its bound was written as (a call, a literal, none): the
// declarations below redeclare one function, overload another, deduce
// the same, and take the same address. kiln keeps the written bound on
// the adjusted type as sugar for tooling clients (spec/types.md 7.2
// "Written bounds", WP-3101); none of this may see it. Reduced from real
// code.
//@ std: c++26
//@ expect: run
//@ stdout: 3 9 2 1 1
extern "C" int printf(const char *, ...);

constexpr int f(int x) { return x; }
int g(int a[f(2)]);
int g(int *a);
int g(int a[7]);
int g(int a[]) { return a[2]; }
int g(long *a) { return static_cast<int>(a[0]); }
static_assert(__is_same(decltype(g(static_cast<int *>(nullptr))), int));

template <class T> int h(T a[f(2)]) { return static_cast<int>(sizeof(T)); }
static_assert(__is_same(decltype(&h<short>), int (*)(short *)));

struct S {
  int m(char b[f(4)]);
};
int S::m(char *b) { return b[0]; }

using P = int (*)(int *);
P gp = g;
P gp2 = static_cast<int (*)(int[f(5)])>(g);

int main() {
  int a[3] = {1, 2, 3};
  long l[1] = {9};
  short s[2] = {};
  char c[4] = {1};
  printf("%d %d %d %d %d\n", g(a), g(l), h(s), S{}.m(c), gp == gp2 ? 1 : 0);
}
