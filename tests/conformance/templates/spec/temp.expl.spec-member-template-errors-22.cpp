// [temp.expl.spec]/16, [dcl.meaning.general]/2: a member template
// specialization `template<> template<class U> R A<long>::m(U)` must
// redeclare a member template of `A<long>`; the class context prints as
// clang's `ak_declcontext` (`A<T>` for a pattern, else scoped:
// `N::A<long>::C`). An invalid out-of-line declaration is not found by
// later ones.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
// clang 22 qualifies the current instantiation and notes the class searched.
//@ diag: 34:48 error "out-of-line definition of 'g' does not match any declaration in 'N::A<long>'"
//@ diag: 24:27 note "A defined here"
//@ diag: 35:48 error "out-of-line definition of 'q' does not match any declaration in 'N::A<long>'"
//@ diag: 24:27 note "A defined here"
//@ diag: 36:34 error "out-of-line definition of 'q' does not match any declaration in 'N::A<T>'"
//@ diag: 24:27 note "A defined here"
//@ diag: 37:52 error "out-of-line definition of 'q' does not match any declaration in 'N::A<T>'"
//@ diag: 24:27 note "A defined here"
//@ diag: 38:30 error "out-of-line definition of 'q' does not match any declaration in 'N::A<long>'"
//@ diag: 24:27 note "A defined here"
//@ diag: 39:33 error "out-of-line definition of 'q' does not match any declaration in 'N::A<long>::C'"
//@ diag: 25:10 note "C defined here"
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
