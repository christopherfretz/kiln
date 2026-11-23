// [class.local], [dcl.fct.default] (GNU statement expressions): the locals
// of a statement expression in a local class's default argument, or in a
// lambda in its default member initializer, are block-scope variables of
// the enclosing function (or of the lambda), not members of the class: a
// `static` one is a static local of the function (mangled as one, shared by
// every use of the default argument), a class declared there is a local
// class, and a lambda there captures them. Reduced from real code (WP-2474).
//@ expect: run
//@ stdout: 5 6 7 | 3 | 2 3 | 9 | 4
#include <cstdio>

int def_arg() {
  struct L {
    int m(int a = ({ static int s = 5; s++; })) { return a; }
    int n(int a = ({ int t = 2; [&] { return t + 1; }(); })) { return a; }
  };
  L l;
  int x = l.m();
  int y = l.m();
  int z = l.m();
  std::printf("%d %d %d | ", x, y, z);
  return l.n();
}

int lambda_in_init() {
  struct L {
    int fl = [] { return ({ int tf = 2; tf; }); }();
    int f2 = [] { return ({ struct In { int v = 3; }; In{}.v; }); }();
  };
  L l;
  std::printf("%d %d | ", l.fl, l.f2);
  return 9;
}

template <class T> T in_template() {
  struct L {
    T m(T a = ({ T t = 4; t; })) { return a; }
  };
  return L{}.m();
}

int main() {
  std::printf("%d | ", def_arg());
  std::printf("%d | ", lambda_in_init());
  std::printf("%d\n", in_template<int>());
}
