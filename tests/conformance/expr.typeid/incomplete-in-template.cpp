// [expr.typeid]/4, [temp.res.general]/6: the operand of 'typeid' naming a
// class that is only declared is ill-formed even in a template, when the
// class is a local class or a member class of the current instantiation
// that has no definition yet. clang diagnoses it where the template is
// defined, once, whether or not the template is instantiated, and later
// definitions do not help; kiln follows (GCC 16.1 waits for the
// instantiation, which [temp.res.general]/6 also allows). An operand that
// is an expression, an injected-class-name, a template-id or a dependent
// name waits for instantiation. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:47 error "'typeid' of incomplete type 'L'"
//@ diag: 23:38 note "forward declaration of 'L'"
//@ diag: 25:47 error "'typeid' of incomplete type 'L'"
//@ diag: 25:38 note "forward declaration of 'L'"
//@ diag: 25:65 error "'typeid' of incomplete type 'L'"
//@ diag: 25:38 note "forward declaration of 'L'"
//@ diag: 26:47 error "'typeid' of incomplete type 'L'"
//@ diag: 26:38 note "forward declaration of 'L'"
//@ diag: 27:58 error "'typeid' of incomplete type 'B'"
//@ diag: 27:38 note "forward declaration of 'A::B'"
#include <typeinfo>
template <class T> void f() { struct L; (void)typeid(L); }
template void f<int>();
template <class T> void g() { struct L; (void)typeid(L&); (void)typeid(const L); }
template <class T> void h() { struct L; (void)typeid(L); struct L {}; }
template <class T> struct A { struct B; void f() { (void)typeid(B); } };
template <class T> struct A<T>::B {};

// Not diagnosed before instantiation: an expression operand, a complete
// local class, a pointer, the injected-class-name, a template-id, a
// dependent name, and 'sizeof'.
template <class T> void k() { struct L; L *p = nullptr; (void)typeid(*p); (void)typeid(L *); }
template <class T> void m() { struct L { T t; }; (void)typeid(L); (void)sizeof(L); }
template void m<int>();
template <class T> struct X;
template <class T> struct C {
  void f() { (void)typeid(C); (void)typeid(X<T>); (void)typeid(typename T::Z); }
};
template <class T> void n() { struct L; (void)sizeof(L); }
