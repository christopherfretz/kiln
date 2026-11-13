// [dcl.attr.deprecated]/4: a deprecated member function template named by
// a member access (`s.f(1)`, `p->g(2)`, an implicit `this->f(1)`) warns for
// the template, then for the specialization, as clang 19 and 22
// (`DiagnoseUseOfDecl` of the found template, then of the specialization);
// named by a qualified name, through an implicit `this` as a static
// member, or as a non-member, only the specialization. WP-1405.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 38:14 warning "'f' is deprecated"
//@ diag: 36:42 note "'f' has been explicitly marked deprecated here"
//@ diag: 38:14 warning "'f<int>' is deprecated"
//@ diag: 36:24 note "'f<int>' has been explicitly marked deprecated here"
//@ diag: 38:20 warning "'g<int>' is deprecated: m"
//@ diag: 37:24 note "'g<int>' has been explicitly marked deprecated here"
//@ diag: 38:32 warning "'f' is deprecated"
//@ diag: 36:42 note "'f' has been explicitly marked deprecated here"
//@ diag: 38:32 warning "'f<int>' is deprecated"
//@ diag: 36:24 note "'f<int>' has been explicitly marked deprecated here"
//@ diag: 42:5 warning "'f' is deprecated"
//@ diag: 36:42 note "'f' has been explicitly marked deprecated here"
//@ diag: 42:5 warning "'f<int>' is deprecated"
//@ diag: 36:24 note "'f<int>' has been explicitly marked deprecated here"
//@ diag: 43:6 warning "'g<int>' is deprecated: m"
//@ diag: 37:24 note "'g<int>' has been explicitly marked deprecated here"
//@ diag: 44:6 warning "'g' is deprecated: m"
//@ diag: 37:54 note "'g' has been explicitly marked deprecated here"
//@ diag: 44:6 warning "'g<int>' is deprecated: m"
//@ diag: 37:24 note "'g<int>' has been explicitly marked deprecated here"
//@ diag: 45:5 warning "'f' is deprecated"
//@ diag: 36:42 note "'f' has been explicitly marked deprecated here"
//@ diag: 45:5 warning "'f<int>' is deprecated"
//@ diag: 36:24 note "'f<int>' has been explicitly marked deprecated here"
//@ diag: 46:3 warning "'h<int>' is deprecated"
//@ diag: 40:22 note "'h<int>' has been explicitly marked deprecated here"
struct S {
  template <class T> [[deprecated]] void f(T) {}
  template <class T> [[deprecated("m")]] static void g(T) {}
  void u() { f(1); g(1); this->f(1); }
};
template <class T> [[deprecated]] void h(T) {}
void use(S s, S *p) {
  s.f(1);
  S::g(1);
  p->g(2);
  s.f<int>(1);
  h(1);
}
