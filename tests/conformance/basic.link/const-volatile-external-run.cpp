// [basic.link]/3.2: a namespace-scope `const volatile` variable is not
// given internal linkage by its type; it is one external object, so its
// address taken in two functions (and through an `extern` redeclaration)
// is the same and its value is the initializer.
//@ expect: run
//@ stdout: 1 3 5 same
#include <cstdio>

const volatile int cv = 1;
extern const volatile int ecv;
const volatile int ecv = 3;
const volatile int arr[2] = {4, 5};

const volatile int *addr1() { return &cv; }
const volatile int *addr2() {
  extern const volatile int cv;
  return &cv;
}
const volatile int *last() { return &arr[1]; }

int main() {
  bool same = addr1() == addr2() && addr1() == &cv;
  std::printf("%d %d %d %s\n", *addr1(), ecv, *last(), same ? "same" : "differ");
  return same ? 0 : 1;
}
