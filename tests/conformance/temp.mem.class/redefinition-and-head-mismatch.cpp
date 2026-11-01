// [temp.mem]/1, [temp.over.link]/6, [basic.def.odr]/1: an out-of-line member class
// definition's template heads must be equivalent to the enclosing templates' heads; a member
// class is defined at most once, and only as the kind of member it was declared as
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:33 error "redefinition of 'B'"
//@ diag: 18:10 note "previous definition is here"
//@ diag: 25:34 error "template parameter has a different kind in template redeclaration"
//@ diag: 20:19 note "previous template declaration is here"
//@ diag: 26:1 error "too many template parameters in template redeclaration"
//@ diag: 17:1 note "previous template declaration is here"
//@ diag: 27:52 error "redefinition of 'G' as different kind of symbol"
//@ diag: 22:10 note "previous definition is here"
//@ diag: 28:33 error "redefinition of 'D' as different kind of symbol"
//@ diag: 20:29 note "previous definition is here"
//@ diag: 29:33 error "no struct named 'Nope' in 'A<T>'"
template <class T> struct A {
  struct B {};
  struct C;
  template <class U> struct D;
  struct E;
  struct G;
};
template <class T> struct A<T>::B {};
template <class T> template <int N> struct A<T>::D {};
template <class T, class U> struct A<T>::E {};
template <class T> template <class U> struct A<T>::G {};
template <class T> struct A<T>::D {};
template <class T> struct A<T>::Nope {};
