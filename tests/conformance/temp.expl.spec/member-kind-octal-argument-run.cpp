// [temp.expl.spec], [temp.names]/3, [lex.icon]: the specialization that
// `R<010>` names is `R<8>`, not `R<10>`, so `cmp1` in `R<010>::cmp1<0>::cmp2
// > args` is a variable and the statement is an expression, not a
// redeclaration of `args`.
//@ expect: run
//@ stdout: expr end
#include <cstdio>

template <int D = 0> struct declared {
  static const int cmp2 = 0;
  friend void operator>(int, const declared &) { std::printf("expr"); }
};
template <int> struct resolve;
template <> struct resolve<8> { static const int cmp1 = 0; };
template <> struct resolve<10> {
  template <int> struct cmp1 { static const int cmp2 = 0; };
};

int main() {
  declared<> args;
  declared<resolve<010>::cmp1<0>::cmp2> args;
  std::printf(" end\n");
}
