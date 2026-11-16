// [dcl.fct.default]/4, /6: a redeclaration adds no default argument to a
// function template, nor in an out-of-line definition of a member of a
// class template, and none that makes a constructor a default, copy or
// move constructor.
// Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:33 error "default arguments cannot be added to a function template that has already been declared"
//@ diag: 18:24 note "previous template declaration is here"
//@ diag: 24:36 error "default arguments cannot be added to an out-of-line definition of a member of a class template"
//@ diag: 25:57 error "default arguments cannot be added to a function template that has already been declared"
//@ diag: 22:26 note "previous template declaration is here"
//@ diag: 27:10 error "addition of default argument on redeclaration makes this constructor a default constructor"
//@ diag: 26:12 note "previous declaration is here"
//@ diag: 28:20 error "addition of default argument on redeclaration makes this constructor a copy constructor"
//@ diag: 26:20 note "previous declaration is here"
template<class T> void t(T, int);
template<class T> void t(T, int = 0) {}
template<class T> struct C {
  void f(int);
  template<class U> void g(U, int);
};
template<class T> void C<T>::f(int = 0) {}
template<class T> template<class U> void C<T>::g(U, int = 1) {}
struct N { N(int); N(const N&, int); };
N::N(int = 0) {}
N::N(const N&, int = 0) {}
