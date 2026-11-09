// [dcl.type.elab]/3, [temp.inst]/3: the forward reference `enum V *pv;` in
// a function template body is reported once, at its definition; the
// instantiation declares the local enumeration ahead of any declaration
// statement of it (clang `FindInstantiatedDecl` `NeedInstantiate`), without
// another diagnostic, and a later definition in the body (g) defines that
// same enumeration. kiln also reported "not yet implemented: reference to
// a member of a dependent context" at each instantiation. WP-1103.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:8 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 20:19 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 23:8 error "ISO C++ forbids forward references to 'enum' types"

template <class T> void f() {
  enum V *pv;
  (void)pv;
  pv = nullptr;
}
template <class T> struct S {
  void m() { enum W *pw = nullptr; (void)pw; }
};
template <class T> int g() {
  enum X *px = nullptr;
  enum X { a = 2 };
  X x = a;
  px = &x;
  return *px;
}
void use() {
  f<int>();
  S<int>().m();
  (void)g<long>();
}
