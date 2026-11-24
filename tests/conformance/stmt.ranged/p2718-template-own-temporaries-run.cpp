// [class.temporary]/7 (C++23, P2718R0), [temp.inst]: the temporaries of a
// for-range-initializer in an instantiated function template persist until
// the end of the loop, each specialization's own, also when the pattern's
// range is type-dependent and the instantiation reuses a non-dependent
// operand that holds them (`T::wrap(id(S(n)))`), and for a default
// argument's temporaries there (`T::wrap(dflt())`). GCC 16.1 agrees; clang
// 19 and 22 do not extend these in a template (spec/sema-class.md §5.4,
// WP-3271). Reduced from real code.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ oracle: gcc
//@ stdout: 1: +1 1 2 -1 | +7 7 8 -7
//@ stdout: 2: +2 2 3 -2 | +7 7 8 -7
//@ stdout: 3: +3 3 4 -3 | +7 7 8 -7
//@ stdout: 4: +4 4 5 -4 | +7 7 8 -7
//@ stdout: 5: +5 5 6 -5 | +7 7 8 -7
//@ skip(clang): clang 19/22 do not extend the temporaries of a for-range-initializer in a template
extern "C" int printf(const char *, ...);
struct S {
  int a[2];
  S(int x) : a{x, x + 1} { printf(" +%d", x); }
  ~S() { printf(" -%d", a[0]); }
  const int *begin() const { return a; }
  const int *end() const { return a + 2; }
};
const S &id(const S &s) { return s; }
const S &dflt(const S &s = S(7)) { return s; }
struct Wrap {
  static const S &wrap(const S &s) { return s; }
};
struct Wrap2 : Wrap {};

template <class T> void plain(int k) {
  printf("%d:", k);
  for (int x : id(S(k))) printf(" %d", x);
  printf(" |");
  for (int x : dflt()) printf(" %d", x);
  printf("\n");
}
template <class T> void dep(int k) {
  printf("%d:", k);
  for (int x : T::wrap(id(S(k)))) printf(" %d", x);
  printf(" |");
  for (int x : T::wrap(dflt())) printf(" %d", x);
  printf("\n");
}

int main() {
  plain<int>(1);
  plain<long>(2);
  dep<Wrap>(3);
  dep<Wrap2>(4);
  dep<Wrap>(5);
}
