// [basic.scope.scope]/4, [basic.lookup.general]/4, [dcl.enum]/11: an
// unscoped enumerator named like an overload set declared earlier in the
// same scope (two functions, or one function template) is not a
// redeclaration conflict in clang (`ActOnEnumConstant` asks
// `LookupSingleName`, which finds no single declaration for an overload
// set): it is declared beside the functions. A qualified name into the
// enumeration names the enumerator; the functions stay callable where
// they were looked up before it. Namespace, class and block scope (WP-929).
//@ expect: run
//@ stdout: 7 22 100 6 5 10 9
#include <cstdio>
int f(int x) { return x + 1; }
int f(double) { return 20; }
int before() { return f(1) + f(1.0); }
enum E { f = 7 };
template <class T> int t(T x) { return int(x) * 3; }
int tbefore() { return t(2); }
enum F { t = 100 };
struct S {
  int m() { return 1; }
  int m(int x) { return x; }
  enum K { m = 5 };
  int use() { return K::m * 2; }
};
int local() {
  void g();
  void g(int);
  enum L { g = 9 };
  return L::g;
}
int main() {
  S s;
  std::printf("%d %d %d %d %d %d %d\n", E::f, before(), F::t, tbefore(), S::K::m, s.use(), local());
}
