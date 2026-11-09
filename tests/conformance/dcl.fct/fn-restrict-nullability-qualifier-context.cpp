// [class.static.mfct]/2, [dcl.fct]/6: a static member function and a
// non-member function have no cv-qualifier-seq; GNU `__restrict` after the
// parameter list counts, and a lone nullability keyword is reported with
// an empty spelling at the start of the declaration (clang). Restrict on
// a variable's placeholder or a parameter's `auto` is "restrict requires
// a pointer or reference ('auto' is invalid)"; a duplicate `__restrict`
// method qualifier is -Wduplicate-decl-specifier (WP-1091).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:20 error "static member function cannot have '__restrict' qualifier"
//@ diag: 22:3 error "static member function cannot have '' qualifier"
//@ diag: 23:20 error "static member function cannot have 'const' qualifier"
//@ diag: 24:23 warning "duplicate 'restrict' declaration specifier"
//@ diag: 25:18 error "class member cannot be redeclared"
//@ diag: 25:8 note "previous declaration is here"
//@ diag: 27:1 error "non-member function cannot have '' qualifier"
//@ diag: 28:10 error "non-member function cannot have 'const __restrict' qualifier"
//@ diag: 30:6 error "restrict requires a pointer or reference ('auto' is invalid)"
//@ diag: 31:13 error "restrict requires a pointer or reference ('auto' is invalid)"
struct A {
  static void s1() __restrict;
  static void s2() _Nonnull;
  static void s3() const _Nonnull;
  void d() __restrict __restrict;
  void e(); void e() __restrict;
};
void f() _Nonnull;
void g() const __restrict;
int i;
auto __restrict v = &i;
void h(auto __restrict x);
