// [temp.expl.spec]/16, [dcl.meaning.general]/2: a member template
// specialization `template<> template<class U> R A<long>::m(U)` must
// redeclare a member template of `A<long>`; the class context prints as
// clang's `ak_declcontext` (`A<T>` for a pattern, else scoped:
// `N::A<long>::C`). An invalid out-of-line declaration is not found by
// later ones.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:48 error "out-of-line definition of 'g' does not match any declaration in 'N::A<long>'"
//@ diag: 27:48 error "out-of-line definition of 'q' does not match any declaration in 'N::A<long>'"
//@ diag: 28:34 error "out-of-line definition of 'q' does not match any declaration in 'A<T>'"
//@ diag: 29:52 error "out-of-line definition of 'q' does not match any declaration in 'A<T>'"
//@ diag: 30:30 error "out-of-line definition of 'q' does not match any declaration in 'N::A<long>'"
//@ diag: 31:33 error "out-of-line definition of 'q' does not match any declaration in 'N::A<long>::C'"
namespace N {
template <class T> struct A {
  struct C {
    void z();
  };
  template <class U> int g(U) { return 1; }
  void f();
};
} // namespace N
int use() { return N::A<long>().g(1); }
template <> template <class U> int N::A<long>::g(U) { return 2; }
template <> template <class U> int N::A<long>::g(U, int) { return 2; }
template <> template <class U> int N::A<long>::q(U) { return 0; }
template <class T> void N::A<T>::q() {}
template <class T> template <class U> int N::A<T>::q(U) { return 0; }
template <> void N::A<long>::q() {}
template <> void N::A<long>::C::q() {}
