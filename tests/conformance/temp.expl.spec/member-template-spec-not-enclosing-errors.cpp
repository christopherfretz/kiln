// [temp.expl.spec]/3, [temp.spec.partial.general]/4, [dcl.meaning.general]/2:
// a member template of an implicit class template specialization, or a
// specialization of one, is declared in the class or in a namespace
// enclosing it. A declarator that is not a template-id is clang's
// diagnoseQualifiedDeclaration (from HandleDeclarator and
// CheckClassTemplate); an explicit or partial specialization of a member
// class, function or variable template is CheckTemplateSpecializationScope,
// against the specialization 'A<long>' (WP-1178). The specializations are
// still declared at namespace scope (clang's recovery).
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 42:50 error "cannot define or redeclare 'C' here because namespace 'M' does not enclose namespace 'A<int>'"
//@ diag: 43:50 error "cannot define or redeclare 'D' here because namespace 'M' does not enclose namespace 'A<int>'"
//@ diag: 44:48 error "cannot define or redeclare 'g' here because namespace 'M' does not enclose namespace 'A<int>'"
//@ diag: 45:41 error "cannot define or redeclare 'g' here because namespace 'M' does not enclose namespace 'A<int>'"
//@ diag: 46:47 error "cannot define or redeclare 'w' here because namespace 'M' does not enclose namespace 'A<int>'"
//@ diag: 48:51 error "class template partial specialization of 'C' not in class 'A<long>' or an enclosing namespace"
//@ diag: 32:29 note "explicitly specialized declaration is here"
//@ diag: 49:44 error "class template specialization of 'C' not in class 'A<long>' or an enclosing namespace"
//@ diag: 32:29 note "explicitly specialized declaration is here"
//@ diag: 50:42 error "function template specialization of 'g' not in class 'A<long>' or an enclosing namespace"
//@ diag: 34:27 note "explicitly specialized declaration is here"
//@ diag: 51:41 error "variable template specialization of 'w' not in class 'A<long>' or an enclosing namespace"
//@ diag: 35:33 note "explicitly specialized declaration is here"
//@ diag: 52:48 error "variable template partial specialization of 'w' not in class 'A<long>' or an enclosing namespace"
//@ diag: 35:33 note "explicitly specialized declaration is here"
//@ diag: 57:41 error "class template specialization of 'C' not in class 'A<char>' or an enclosing namespace"
//@ diag: 32:29 note "explicitly specialized declaration is here"
namespace N {
template <class T> struct A {
  template <class X> struct C;
  template <class X> struct D;
  template <class X> void g(X);
  template <class X> static int w;
};
inline namespace L {}
} // namespace N

namespace M {
// Not template-ids: diagnoseQualifiedDeclaration.
template <> template <class X> struct N::A<int>::C {};
template <> template <class X> struct N::A<int>::D;
template <> template <class X> void N::A<int>::g(X) {}
template <> template <> void N::A<int>::g(char) {}
template <> template <class X> int N::A<int>::w = 2;
// Template-ids: CheckTemplateSpecializationScope, against 'A<long>'.
template <> template <class X> struct N::A<long>::C<X *> { int c = 1; };
template <> template <> struct N::A<long>::C<int> { int c = 2; };
template <> template <> void N::A<long>::g<int>(int) {}
template <> template <> int N::A<long>::w<int> = 3;
template <> template <class X> int N::A<long>::w<X *> = 4;
} // namespace M

namespace N {
inline namespace L {
template <> template <> struct A<char>::C<int> {};
} // namespace L
} // namespace N

// Recovery: at namespace scope each specialization is still declared.
int c = N::A<long>::C<int>{}.c + N::A<long>::C<int *>{}.c;
int w = N::A<long>::w<int> + N::A<long>::w<int *>;
void h() { N::A<long>{}.g(1); }
