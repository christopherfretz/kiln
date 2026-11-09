// [dcl.fct]/6: a member function with an explicit object parameter has
// no cv-qualifier-seq; GNU `__restrict` after the parameter list is one
// ("explicit object member function cannot have '__restrict' qualifier"
// at it), and a lone nullability keyword there is reported with an empty
// spelling at the start of the declaration, as clang does (clang test
// SemaCXX/cxx2b-deducing-this.cpp:27-28). A placeholder is no pointer:
// `this auto __restrict self` is "restrict requires a pointer or
// reference ('auto' is invalid)" (WP-1091).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:21 error "explicit object member function cannot have '__restrict' qualifier"
//@ diag: 23:3 error "explicit object member function cannot have '' qualifier"
//@ diag: 24:18 error "explicit object member function cannot have '__restrict' qualifier"
//@ diag: 25:19 error "explicit object member function cannot have 'const __restrict' qualifier"
//@ diag: 26:3 error "explicit object member function cannot have '' qualifier"
//@ diag: 27:18 error "explicit object member function cannot have 'const' qualifier"
//@ diag: 28:18 error "explicit object member function cannot have '__restrict &' qualifier"
//@ diag: 29:20 error "restrict requires a pointer or reference ('auto' is invalid)"
//@ diag: 30:22 error "restrict requires a pointer or reference ('C auto' is invalid)"
template<class T> concept C = true;
struct S {
  void k(this auto) __restrict;
  void l(this auto) _Nonnull;
  void m(this S) __restrict;
  void n(this S&) const __restrict;
  void o(this S) _Nullable;
  void p(this S) const _Nonnull;
  void q(this S) __restrict__ &;
  void r(this auto __restrict self);
  void s(this C auto __restrict self);
};
