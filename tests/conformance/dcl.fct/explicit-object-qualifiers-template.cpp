// [dcl.fct]/6: an explicit object member function has no cv-qualifier-seq
// or ref-qualifier. clang reports them at the first qualifier written,
// before an explicit object parameter of a static or virtual member, and
// drops them from the type, so an instantiation of a class template does
// not report them again. A friend reports them as a non-member function's.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:19 error "explicit object member function cannot have 'const' qualifier"
//@ diag: 24:38 error "explicit object member function cannot have '&' qualifier"
//@ diag: 25:19 error "explicit object member function cannot have 'const' qualifier"
//@ diag: 26:19 error "explicit object member function cannot have 'volatile' qualifier"
//@ diag: 27:19 error "explicit object member function cannot have 'const &&' qualifier"
//@ diag: 28:27 error "explicit object member function cannot have 'const' qualifier"
//@ diag: 28:18 error "an explicit object parameter cannot appear in a virtual function"
//@ diag: 29:26 error "explicit object member function cannot have 'const' qualifier"
//@ diag: 29:17 error "an explicit object parameter cannot appear in a static function"
//@ diag: 31:41 error "explicit object member function cannot have 'const' qualifier"
//@ diag: 33:27 error "non-member function cannot have 'const' qualifier"
//@ diag: 33:18 error "an explicit object parameter cannot appear in a non-member function"
//@ diag: 35:13 error "an explicit object parameter cannot appear in a non-member function"
//@ diag: 35:23 error "non-member function cannot have 'const' qualifier"
template<class T> struct A {
  void f(this A&) const;
  int operator()(this A const&, int) &;
  auto g(this A&) const -> int;
  void h(this A&) volatile requires true;
  void k(this A&) const && = delete;
  virtual void v(this A&) const;
  static void s(this A&) const;
};
template<class T> void A<T>::f(this A&) const {}
struct B {
  friend void fr(this B&) const;
};
void free_f(this int) const;
A<int> a;
