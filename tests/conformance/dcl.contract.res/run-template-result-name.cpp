// [dcl.contract.res]/1, [temp.inst] (P2900R14): the result name of a
// postcondition of a templated function whose predicate is neither type-
// nor value-dependent (`int f(int) post(r: r > 4)` in a class template or
// a function template) names the result of each specialization; kiln
// failed to compile it ("result name outside a postcondition"). The
// violation is observed by the default handler (on stderr) and the
// program goes on. -fcontracts also links libstdc++'s handler where the
// library has one, as g++ does. Found running GCC's C++ testsuite
// (g++.dg/contracts).
//@ expect: run
//@ args: -fcontracts -fcontract-evaluation-semantic=observe
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: 5 1 7 2
extern "C" int printf(const char*, ...);
template <class T> struct TS {
  int f(int a) post(r: r > 4) { return a; }
  template <class U> int g(U a) post(r: r > 4) { return a; }
};
template <class T> int h(int a) post(r: r > 4) { return a; }
int main() {
  TS<int> ts;
  printf("%d %d %d %d\n", ts.f(5), ts.f(1), ts.g(7), h<long>(2));
}
