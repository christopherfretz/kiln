// [dcl.attr.deprecated]/4 (WP-297, clang ShouldDiagnoseAvailabilityInContext):
// no warning for a use inside a deprecated entity: a deprecated function's
// body, a deprecated class's members, a deprecated variable's type.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 22:12 warning "'S' is deprecated"
//@ diag: 12:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 23:12 warning "'S' is deprecated"
//@ diag: 12:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 23:23 warning "'a' is deprecated"
//@ diag: 16:3 note "'a' has been explicitly marked deprecated here"
struct [[deprecated]] S {
  S& operator=(const S&);
  static S make();
};
[[deprecated]] int a;
[[deprecated]] void g(S);
[[deprecated]] S gv;
[[deprecated]] void f() { (void)a; S s; }
struct [[deprecated]] C { int m = a; void k() { (void)a; S t; } };
template<class T> struct [[deprecated]] TP { TP* self; void z(TP<T>&); };
struct D : S {};
void h() { S s; (void)a; }
