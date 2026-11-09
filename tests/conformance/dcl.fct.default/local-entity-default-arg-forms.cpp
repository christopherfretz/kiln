// [dcl.fct.default]/8-9: `this` may not appear in a default argument (not
// even unevaluated), a lambda-expression in one captures nothing, and a
// local variable (static, thread_local or a reference included) does not
// appear potentially evaluated; an odr-use of a constant (`&c`, binding a
// reference) counts. Every violation is reported. Valid pair:
// local-entity-default-arg-valid.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 28:25 error "default argument references 'this'"
//@ diag: 29:32 error "default argument references 'this'"
//@ diag: 30:26 error "lambda expression in default argument cannot capture any entity"
//@ diag: 36:24 error "default argument references local variable 'r' of enclosing function"
//@ diag: 38:33 error "default argument references local variable 'c' of enclosing function"
//@ diag: 39:32 error "default argument references local variable 'c' of enclosing function"
//@ diag: 40:25 error "lambda expression in default argument cannot capture any entity"
//@ diag: 41:37 error "lambda expression in default argument cannot capture any entity"
//@ diag: 43:24 error "default argument references local variable 'si' of enclosing function"
//@ diag: 45:24 error "default argument references local variable 'tl' of enclosing function"
//@ diag: 46:29 error "default argument references local variable 'i' of enclosing function"
//@ diag: 47:29 error "default argument references local variable 'i' of enclosing function"
//@ diag: 48:24 error "default argument references local variable 'i' of enclosing function"
//@ diag: 48:28 error "default argument references local variable 'r' of enclosing function"
//@ diag: 53:23 error "default argument references local variable 'i' of enclosing function"
//@ diag: 54:21 error "default argument references local variable 'ci' of enclosing function"
struct A {
  int n;
  void f() {
    extern void g(int = this->n);
    extern void k(int = sizeof(this->n));
    extern void m(int = [this] { return 1; }());
  }
};
void f() {
  int i = 0;
  int &r = i;
  extern void h1(int = r);
  constexpr int c = 3;
  extern void h2(const int * = &c);
  extern void h3(const int & = c);
  extern void h4(int = [i] { return i; }());
  extern void h5(int = [=] { return i; }());
  static int si = 0;
  extern void h6(int = si);
  thread_local int tl = 0;
  extern void h7(int = tl);
  struct L { void m(int x = i); };
  extern void h8(int = [x = i] { return x; }());
  extern void h9(int = i + r);
}
template <class T> void tf() {
  T i = 0;
  const int ci = 1;
  extern void w(int = i);
  extern void y(T = ci);
}
