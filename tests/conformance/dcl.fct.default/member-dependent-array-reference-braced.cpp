//@ expect: run
//@ stdout: 0 2 1 3
// WP-782: [dcl.fct.default]/5, [temp.inst]/12: the braced default
// argument `= {}` of a member function of a class template whose
// parameter is a reference to an array of dependent bound is
// copy-list-initialized when instantiated, binding to a temporary
// `const O[N ? N : 1]` ([dcl.init.list]/3.10), never to an array of no
// elements (LLVM's Option/ArgList.h `arg_iterator`).
#include <cstdio>

struct O {
  int v = 0;
};
template <unsigned N> struct It {
  int n;
  It(int, const O (&ids)[N ? N : 1] = {}) : n(sizeof(ids) / sizeof(O) - 1 + ids[0].v) {}
  static int f(const O (&ids)[N] = {{1}}) { return ids[0].v + N; }
};
template <class T, unsigned N> struct Arr {
  static unsigned g(const T (&a)[N] = {}) { return sizeof(a) / sizeof(T); }
};

int main() {
  std::printf("%d %d %d %u\n", It<0>(1).n, It<3>(1).n, It<0 + 2>::f() - 2,
              Arr<char, 3>::g());
}
