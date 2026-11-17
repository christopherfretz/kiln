//@ expect: ast
//@ skip(clang): clang 22 treats a call through `this` of the current instantiation as type-dependent
//@ oracle: none
// [temp.dep.expr]/5: a call through `this` of the current instantiation
// is not type-dependent when it names member functions whose types are
// not dependent, of a class with no dependent base, and no argument is
// type-dependent: `this->f(1)`, `f(1)` and `Q::g()` have the return type
// (an lvalue for an lvalue reference return), and so do `this->ov(1)`
// and `this->tm(1)`, whose overload resolution (and deduction) succeeds
// at definition (WP-1609). Every other call stays `<dependent type>` and
// is decided at instantiation: a deduced return type, a dependent
// function type, a type-dependent argument, a result naming an incomplete class (which may
// be complete at instantiation), and any call in a class with a dependent
// base (which may hide or add members at instantiation).
struct Inc;
struct Q { int g() { return 0; } };
template <class T> struct A : Q {
  long f(int) { return 1; }
  long &r() { static long l; return l; }
  int ov(int) { return 0; }
  int ov(long) { return 0; }
  template <class U> int tm(U) { return 0; }
  auto d() { return 1; }
  T dt() { return T(); }
  Inc *inc();
  void use(T t) {
    this->f(1);
    f(1);
    Q::g();
    r();
    this->ov(1);
    this->tm(1);
    d();
    dt();
    f(t);
    inc();
  }
};
template <class T> struct D : T {
  long f(int) { return 1; }
  void use() { f(1); }
};
