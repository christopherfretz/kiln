// [dcl.init.list]/3.2, /3.10: a braced list with one element of the
// parameter's class initializes the parameter from that element; the
// list, braces included, is the argument (spec/ast.md §21.2): a
// reference parameter binds to the element through the list, a class
// parameter is copied from it; a variable's direct-list-initialization
// begins at its name. Reduced from real code.
//@ expect: compile-ok
struct O {
  bool a = false;
  long b = 0;
};
struct P : O {};
int f(const O &);
int g(O);
struct N {
  N(int);
  N(const N &);
  int v;
};
int k(N);
int use(O x, P p, N n) {
  O v{x};
  O w = {p};
  N m{n};
  N one{1};
  return f({x}) + g({x}) + f({p}) + g({p}) + g({O{x}}) + k({n}) + k({1}) + v.a + w.a + m.v + one.v;
}
template <class T> int inst(T x) {
  T v{x};
  return f({x}) + g({x}) + g({T{x}}) + v.a;
}
int instantiated = inst(O{});
