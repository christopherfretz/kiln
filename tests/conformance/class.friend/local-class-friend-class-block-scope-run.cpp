// [class.friend]/11: `friend class F;` in a local class looks for a prior
// declaration only up to the innermost enclosing non-class scope; without
// one, `F` belongs to that block scope, and the class `F` defined later in
// the block is the friend. kiln declared `F` in the enclosing namespace,
// so the block's `F` was not a friend ("'i' is a private member of 'L'";
// found running GCC's C++ testsuite, which GCC accepts). Also: a class
// declared earlier in the block, a member class of an enclosing local
// class, a nested block's own class, an elaborated `class F *p;` in the
// block before the definition, a lambda body, and a function template
// (the friend declared again in each instantiated block). The
// -denied sibling shows the classes outside the block it leaves out.
//@ skip(clang): clang searches past the block scope and does not befriend a class defined later in it
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7 8
extern "C" int printf(const char *, ...);
int later() {
  class L { friend class F; int i = 1; };
  class F { public: int g(L &l) { return l.i; } };
  L l;
  return F().g(l);
}
int earlier() {
  struct B;
  class L { friend struct B; int i = 2; };
  struct B { int g(L &l) { return l.i; } };
  L l;
  return B().g(l);
}
int member() {
  struct O {
    struct X;
    class L { friend struct X; int i = 3; };
    struct X { int g(L &l) { return l.i; } };
  };
  O::L l;
  return O::X().g(l);
}
int nested() {
  struct B { int g() { return 0; } };
  {
    class L { friend struct B; int i = 4; };
    struct B { int g(L &l) { return l.i; } };
    L l;
    return B().g(l);
  }
}
int elaborated() {
  class L { friend class P; int i = 5; };
  class P *ptr = nullptr;
  class P { public: int g(L &l) { return l.i; } };
  L l;
  P p;
  ptr = &p;
  return ptr->g(l);
}
int lambda() {
  auto f = [] {
    class M { friend class N; int j = 6; };
    class N { public: int g(M &m) { return m.j; } };
    M m;
    return N().g(m);
  };
  return f();
}
template <class T> T templ(T t) {
  class L { friend class F; T i; public: L(T x) : i(x) {} };
  class F { public: T g(L &l) { return l.i; } };
  L l(t);
  return F().g(l);
}
int main() {
  printf("%d %d %d %d %d %d %d %d\n", later(), earlier(), member(), nested(), elaborated(),
         lambda(), templ(7), int(templ(8L)));
}
