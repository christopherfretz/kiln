// [class.protected]/1, [expr.unary.op]/3: `&B::m<int>` forms a pointer to
// member, so a protected non-static member template of a base named
// through the base is inaccessible in a derived class, exactly as `&B::f`
// is; named through the derived class (or the member being static) it is
// fine. kiln checked a member template-id as if it were named through
// `*this`, also with dependent template arguments, with a target type or
// an overload set of templates, and in a class template (found running
// GCC's C++ testsuite). GCC 16.1 accepts these; clang 22 rejects them.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 43:18 error "'m' is a protected member of 'B'"
//@ diag: 29:26 note "must name member using the type of the current context 'D'"
//@ diag: 44:37 error "'m' is a protected member of 'B'"
//@ diag: 29:26 note "must name member using the type of the current context 'D'"
//@ diag: 45:33 error "'o' is a protected member of 'B'"
//@ diag: 31:26 note "must name member using the type of the current context 'D'"
//@ diag: 46:18 error "'f' is a protected member of 'B'"
//@ diag: 32:7 note "must name member using the type of the current context 'D'"
//@ diag: 57:29 error "'m' is a protected member of 'B'"
//@ diag: 29:26 note "must name member using the type of the current context 'DT"
//@ diag: 50:18 error "'m' is a protected member of 'B'"
//@ diag: 60:7 note "in instantiation of function template specialization 'D::dep<int>' requested here"
//@ diag: 29:26 note "must name member using the type of the current context 'D'"
//@ diag: 51:35 error "'m' is a protected member of 'B'"
//@ diag: 29:26 note "must name member using the type of the current context 'D'"
class B {
protected:
  template <class U> static int s(U) { return 0; }
  template <class U> int m(U) { return 0; }
  template <class U> int o(U) { return 0; }
  template <class U> int o(U, U) { return 0; }
  int f(int) { return 0; }
};
struct D : B {
  void ok() {
    auto a = &D::m<int>;
    auto b = &B::s<int>;
    int (B::*c)(int) = &D::m<int>;
    int (D::*d)(int, int) = &D::o<int>;
    (void)a; (void)b; (void)c; (void)d;
  }
  void bad() {
    auto a = &B::m<int>;
    int (B::*c)(int) = &B::template m<int>;
    int (B::*d)(int, int) = &B::o<int>;
    auto e = &B::f;
    (void)a; (void)c; (void)d; (void)e;
  }
  template <class T> void dep() {
    auto a = &B::m<T>;
    int (B::*b)(T) = &B::template m<T>;
    (void)a; (void)b;
  }
};
template <class T> struct DT : B {
  void ok() { auto a = &DT::template m<T>; (void)a; }
  void bad() { auto a = &B::m<int>; (void)a; }
};
void use() {
  D().dep<int>();
  DT<int>().ok();
}
