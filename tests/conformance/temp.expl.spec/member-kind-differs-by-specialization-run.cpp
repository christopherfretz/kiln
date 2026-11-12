// [temp.expl.spec], [temp.names]/3: in `R<args>::C<0>::v`, whether `C` is a
// member template, and so whether `<` begins a template argument list,
// depends on the specialization `R<args>` names. Two explicit
// specializations of an undefined primary template declare `C` differently;
// the first is a variable. Boost.ScopeExit's `declared<resolve<sizeof(args)>
// ::cmp1<0>::cmp2> args;` is a declaration when `args` names the namespace-
// scope `undeclared` and the expression `declared<0>::cmp2 > args` when it
// names an enclosing `declared<>` (BOOST_SCOPE_EXIT nested in another).
//@ expect: run
//@ stdout: 1 2 decl expr
#include <cstdio>

template <int> struct R;
template <> struct R<8> { static const int C = 0; };
template <> struct R<16> {
  template <int> struct C { static const int v = 1; };
};
template <int N = 0> struct W {
  static const int v = N + 1;
};
W<R<16>::C<0>::v> w;

template <int D = 0> struct declared {
  void *value;
  static const int cmp2 = 0;
  friend void operator>(int, const declared &) { std::printf(" expr"); }
};
struct undeclared {
  declared<> dummy[2];
};
template <int> struct resolve;
template <> struct resolve<sizeof(declared<>)> { static const int cmp1 = 0; };
template <> struct resolve<sizeof(undeclared)> {
  template <int> struct cmp1 { static const int cmp2 = 0; };
};
extern undeclared args;

int main() {
  std::printf("%d %d", R<16>::C<0>::v, w.v);
  declared<resolve<sizeof(args)>::cmp1<0>::cmp2> args;
  args.value = &args;
  std::printf(" decl");
  {
    declared<resolve<sizeof(args)>::cmp1<0>::cmp2> args;
  }
  std::printf("\n");
}
