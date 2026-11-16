// [namespace.udecl]/14 (and /4): the implicitly-declared copy or move
// assignment operator of the derived class hides a base `operator=` named by
// a using-declarator when their parameter-type-lists correspond; the other
// base assignment operators stay visible. Found running GCC's C++ testsuite
// (WP-1467).
struct A;
struct B {
  constexpr A &operator=(const A &);
  B &operator=(int) { return *this; }
};
// The implicit copy assignment of A hides B's `operator=(const A&)`, so
// `a = a` is not ambiguous; `operator=(int)` stays visible.
struct A : B {
  using B::operator=;
};
void f(A &a, A &b) {
  a = b;
  a = 1;
}
struct C {
  C &operator=(const C &) = delete;
  template <class T> C &operator=(T *) { return *this; }
};
struct D : C {
  using C::operator=;
};
void g(D &d, int *p) { d = p; }
