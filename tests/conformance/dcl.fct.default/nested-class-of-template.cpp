// [dcl.fct.default]/5, [temp.dep.type]/7, [temp.inst]/12: a nested class
// of a class template is a dependent type, so a default argument of that
// type (or a reference to it) is checked only when it is instantiated,
// where the class is complete. Its default member initializers and
// constructors are not needed in the template definition (an `= {}`
// there is not "needed within the definition of the enclosing class").
//@ expect: run
//@ stdout: 7 3 14 5
extern "C" int printf(const char *, ...);
struct Res {
  int n;
  Res() : n(0) {}
  Res(int v) : n(v) {}
};
template<class S> struct L {
  struct H {
    Res value{};
    int n = 7;
  };
  struct K {
    int v;
  };
  static int r(const H &o = {}) { return o.n + o.value.n; }
  static int q(H o = H{{}, 3}) { return o.n; }
  static int s(H o = {Res(10), 4}) { return o.n + o.value.n; }
  static int t(K k = {5}, const K &u = {}) { return k.v + u.v; }
};
int main() {
  printf("%d %d %d %d\n", L<int>::r(), L<int>::q(), L<int>::s(), L<int>::t());
}
