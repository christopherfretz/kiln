// [dcl.fct]/6: a member function with an explicit object parameter has no
// cv-qualifier-seq or ref-qualifier. clang quotes all of them (cv first)
// and points at the first one written.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:18 error "explicit object member function cannot have 'const volatile' qualifier"
//@ diag: 14:18 error "explicit object member function cannot have 'const volatile' qualifier"
//@ diag: 15:18 error "explicit object member function cannot have 'const &' qualifier"
//@ diag: 16:18 error "explicit object member function cannot have 'volatile &&' qualifier"
//@ diag: 17:18 error "explicit object member function cannot have 'const' qualifier"
//@ diag: 18:19 error "explicit object member function cannot have '&' qualifier"
struct S {
  void a(this S) const volatile;
  void b(this S) volatile const;
  void c(this S) const &;
  void d(this S) volatile &&;
  void e(this S) const noexcept;
  void f(this S&) &;
};
