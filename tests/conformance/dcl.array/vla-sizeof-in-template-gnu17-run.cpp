//@ expect: run
//@ strict-diags
//@ args: -std=gnu++17
//@ stdout: 12 8 20 1
// `vla-sizeof-in-template-run.cpp` at -std=gnu++17: a variable length
// array type in a function template is rebuilt in each specialization
// with its own bound, also where it does not depend on a template
// parameter, and also as a namespace-scope initializer's callee.
// Found running GCC's C++ testsuite.
extern "C" int printf(const char*, ...);

template <bool> static int label(int w) {
  (void)sizeof(int[w]);
  return int(sizeof(int[w]));
}
int at_startup = label<false>(3);

template <class T> int in_generic_lambda(int w) {
  auto l = [&](auto x) { return int(sizeof(T[w]) + sizeof(x) - sizeof(x)); };
  return l(0);
}

static int calls;
template <class T> int once(int w) {
  using A = T[++calls + w];
  return int(sizeof(A) + sizeof(A) - sizeof(A));
}

int main() {
  int r = once<int>(4);
  printf("%d %d %d %d\n", at_startup, in_generic_lambda<short>(4), r, calls + label<true>(0));
}
