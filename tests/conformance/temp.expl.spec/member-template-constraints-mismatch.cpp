// [temp.expl.spec]/16, [temp.over.link]/6: a redeclaration of a member
// template of a class template specialization with a different constraint
// matches no member (WP-958).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 12:50 error "out-of-line declaration of 'f' does not match any declaration in 'A<short>'"
template <typename T> concept C = sizeof(T) <= sizeof(long);
template <typename T> concept D = sizeof(T) >= 1;
template <typename T> struct A {
  template <typename U> void f(U) requires C<U>;
};
template <> template <typename U> void A<short>::f(U) requires D<U>;
