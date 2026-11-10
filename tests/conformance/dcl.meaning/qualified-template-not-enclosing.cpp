// [dcl.meaning.general]/2, [namespace.memdef]/2, [class.mfct]/2: a
// qualified definition or redeclaration must be in a scope that encloses
// its qualifier's, also when it is a template: a function template, a
// member of a class template, a member function template, a variable
// template, a static data member of a class template, a class template,
// a member class and a nested class template; non-template classes and
// enumerations too. Only a template-id declarator is exempt (checked as a
// specialization). clang's diagnoseQualifiedDeclaration, called from
// HandleDeclarator, ActOnTag and CheckClassTemplate (WP-1174).
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 44:28 error "cannot define or redeclare 'f' here because namespace 'B' does not enclose namespace 'A'"
//@ diag: 45:34 error "cannot define or redeclare 'g' here because namespace 'B' does not enclose namespace 'S'"
//@ diag: 46:53 error "cannot define or redeclare 'h' here because namespace 'B' does not enclose namespace 'S'"
//@ diag: 47:27 error "cannot define or redeclare 'vt' here because namespace 'B' does not enclose namespace 'A'"
//@ diag: 48:33 error "cannot define or redeclare 'v' here because namespace 'B' does not enclose namespace 'S'"
//@ diag: 49:30 error "cannot define or redeclare 'C' here because namespace 'B' does not enclose namespace 'A'"
//@ diag: 50:36 error "cannot define or redeclare 'N' here because namespace 'B' does not enclose namespace 'S'"
//@ diag: 51:55 error "cannot define or redeclare 'M' here because namespace 'B' does not enclose namespace 'S'"
//@ diag: 52:11 error "cannot define or redeclare 'Z' here because namespace 'B' does not enclose namespace 'A'"
//@ diag: 53:15 error "cannot define or redeclare 'E' here because namespace 'B' does not enclose namespace 'A'"
//@ diag: 54:21 error "cannot define or redeclare 'ded' here because namespace 'B' does not enclose namespace 'A'"
//@ diag: 55:10 error "definition or redeclaration of 'G' cannot name the global scope"
//@ diag: 58:10 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 58:13 error "definition or redeclaration of 'Z' not allowed inside a function"
namespace A {
template <class T> void f();
template <class T> struct S {
  void g();
  template <class U> void h();
  static int v;
  struct N;
  template <class U> struct M;
};
template <class T> extern int vt;
template <class T> struct C;
struct Z;
enum class E : int;
template <class T> void ded(T);
} // namespace A
struct G;
namespace B {
template <class T> void A::f() {}
template <class T> void A::S<T>::g() {}
template <class T> template <class U> void A::S<T>::h() {}
template <class T> int A::vt = 0;
template <class T> int A::S<T>::v = 0;
template <class T> struct A::C {};
template <class T> struct A::S<T>::N {};
template <class T> template <class U> struct A::S<T>::M {};
struct A::Z {};
enum class A::E : int { e };
template <> void A::ded(int) {}
struct ::G {};
} // namespace B
void fn() {
  struct A::Z;
}
