// [temp.expl.spec]/16: a member template of an explicitly specialized class
// template specialization may be declared with its requires-clause. From
// clang's CXX/temp/temp.spec/temp.expl.spec/p8.cpp. Found by the clang
// negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, the constrained member template redeclaration is not matched
//@ expect: compile-ok
template <typename T> concept C = sizeof(T) <= sizeof(long);
template <typename T> struct A {
  template <typename U> void f(U) requires C<U>;
};
template <>
template <typename U>
void A<short>::f(U) requires C<U>;
