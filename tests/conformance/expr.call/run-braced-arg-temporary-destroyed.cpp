// [expr.call]/7, [class.temporary]/4, [dcl.init.list]/3: a by-value class
// parameter copy-list-initialized from a braced-init-list argument is
// destroyed at the end of the full-expression containing the call, the same
// as one initialized from a prvalue (the caller destroys it under the
// Itanium ABI). This covers initializer-list constructors, plain
// constructors, empty braces, default arguments, constructor, member and
// operator calls, and calls inside templates.
//@ expect: run
//@ strict-diags
//@ stdout: 0 0 0 0 0 0 0 0 0 0 0 | 3 12
#include <initializer_list>
extern "C" int printf(const char *, ...);
int live = 0;
struct N {
  N(int = 0, int = 0) { ++live; }
  N(const N &) { ++live; }
  ~N() { --live; }
};
struct M {
  M(std::initializer_list<int>) { ++live; }
  M(const M &) { ++live; }
  ~M() { --live; }
};
void f(N) {}
void g(M) {}
void d(N = {1, 2}) {}
void dl(M = {1, 2, 3}) {}
template <class T> void tf(T, M) {}
struct S {
  S(N, M) {}
  void mf(N) {}
  S &operator=(N) { return *this; }
  void operator()(M) {}
  int operator[](N) { return 0; }
};

// Constant evaluation: the bound temporary's destructor runs too.
struct C {
  int *p;
  constexpr C(int *q, int) : p(q) { ++*p; }
  constexpr ~C() { *p += 10; }
};
constexpr int take(C c) { return *c.p; }
constexpr int probe() {
  int n = 2;
  int during = take({&n, 0});
  return during * 4 + (n == 13 ? 0 : 100);
}
static_assert(probe() == 12);

int main() {
  f({1, 2}); printf("%d ", live);
  f({}); printf("%d ", live);
  g({1, 2}); printf("%d ", live);
  d(); printf("%d ", live);
  dl(); printf("%d ", live);
  tf(1, {1, 2}); printf("%d ", live);
  S s({1, 2}, {1}); printf("%d ", live);
  s.mf({1, 2}); printf("%d ", live);
  s = {1, 2}; printf("%d ", live);
  s({1, 2}); printf("%d ", live);
  s[{1, 2}]; printf("%d |", live);
  int n = 2;
  int during = take({&n, 0});
  printf(" %d %d\n", during, during * 4);
  return n == 13 ? 0 : 1;
}
