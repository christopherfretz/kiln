// [namespace.udecl]/3: the nested-name-specifier of a member
// using-declarator names a base class of the class being defined. Naming
// the class template's own current instantiation `D<T>` is ill-formed, and
// when its dependent bases leave lookup with nothing (no member of that
// name is declared in `D` itself) clang reports "no member named" at the
// template definition. kiln reported only "using declaration refers to its
// own class" at instantiation, for the C++98 access declaration `D<T>::m;`
// and the using-declarations alike (found running GCC's C++ testsuite).
// The accepted side: a base class named in the nested-name-specifier.
//@ expect: compile-fail
//@ std: c++98
//@ strict-diags
//@ diag: 22:9 warning "access declarations are deprecated; use using declarations instead"
//@ diag: 22:9 error "no member named 'zzz' in 'D<T>'"
//@ diag: 23:15 error "no member named 'yyy' in 'D<T>'"
//@ diag: 24:12 error "no member named 'xxx' in 'D<T>'"
//@ diag: 25:9 warning "access declarations are deprecated; use using declarations instead"

template <class T> struct I { void g(); int h; };
template <class T> struct D : I<T> {
  void bar();
  D<T>::zzz;
  using D<T>::yyy;
  using D::xxx;
  I<T>::g;
  using I<T>::h;
};
D<int> d;
