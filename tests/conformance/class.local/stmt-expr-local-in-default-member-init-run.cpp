// [class.local], [class.mem.general] (GNU statement expressions): the
// locals of a statement expression in a local class's default member
// initializer are block-scope variables of the enclosing function, not
// static data members of the class, and naming them in the same
// initializer is no reference to an enclosing function's local: the
// initializer runs in each constructor that uses it, with those locals
// in the constructor's frame. The statement expression has the class's
// access and `this`, a lambda in it captures its locals, a class declared
// in it is a local class, and a `static` local is one of the function. In
// a function template the locals are instantiated as locals too.
// Reduced from real code (WP-2474). Clang declares the locals but reports
// each use from the implicit constructor as a reference to a local of
// the enclosing function, declares them static data members when it
// instantiates the class, and crashes on the private-member case; GCC
// rejects a statement expression in any default member initializer.
//@ expect: run
//@ skip(clang): clang rejects the constructors that use these initializers
//@ skip(gcc): GCC rejects statement expressions outside function bodies
//@ stdout: 2 3 | 15 | 2 3 4 5 | 14 14
#include <cstdio>

int plain() {
  struct L {
    int fl = ({ int tf = 2; tf; });
    int f2 = ({ int tf = 2; tf + 1; });
    L() {}
  };
  L l;
  std::printf("%d %d | ", l.fl, l.f2);
  return 0;
}

int members() {
  struct L {
  private:
    int p = 4;
    static int sp() { return 5; }
    struct PN { int v = 6; };
  public:
    int fl = ({ int tf = p + sp() + PN{}.v; tf; });
  };
  return L{}.fl;
}

int nested() {
  struct L {
    int f1 = ({ int tq = 2; [&] { return tq; }(); });
    int f2 = ({ int tq = 1; [=] { return tq + 2; }(); });
    int f3 = ({ struct In { int z = 4; }; In{}.z; });
    int f4 = ({ static int s = 5; [] { return s; }(); });
  };
  L l;
  std::printf("%d %d %d %d | ", l.f1, l.f2, l.f3, l.f4);
  return 0;
}

template <class T> T in_template() {
  struct L {
    T fl = ({ T tf = 2; tf; });
    T m(T a = ({ T tq = 5; tq; })) { return a; }
    T f2 = ({ T tq = 3; [&] { return tq; }(); });
    T f3 = [] { return ({ T tz = 4; tz; }); }();
  };
  L l;
  return l.fl + l.m() + l.f2 + l.f3;
}

int main() {
  plain();
  std::printf("%d | ", members());
  nested();
  int a = in_template<int>();
  long b = in_template<long>();
  std::printf("%d %ld\n", a, b);
}
