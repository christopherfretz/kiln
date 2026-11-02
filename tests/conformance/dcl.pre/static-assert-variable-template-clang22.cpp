// [dcl.pre]/12, [temp.names]/1: the failed requirement of a variable
// template-id prints its nested-name-specifier and arguments; a
// substituted `T::` prints the bare class, `A::` the specialization
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:28 error "static assertion failed due to requirement 'A<int>::v<int>'"
//@ diag: 33:23 note "in instantiation of member function 'A<int>::h' requested here"
//@ diag: 31:52 error "static assertion failed due to requirement 'A<int>::y'"
//@ diag: 34:15 error "static assertion failed due to requirement 'ns::f<int>'"
//@ diag: 35:15 error "static assertion failed due to requirement '::ns::f<int *>'"
//@ diag: 36:15 error "static assertion failed due to requirement 'ns::S::z<char>'"
//@ diag: 37:15 error "static assertion failed due to requirement 'A<int>::v<long>'"
//@ diag: 38:15 error "static assertion failed due to requirement 'ns::e<int, char>'"
//@ diag: 40:26 error "static assertion failed due to requirement 'ns::S::z<ns::S>'"
//@ diag: 41:15 note "in instantiation of function template specialization 'g<ns::S>' requested here"
//@ diag: 40:54 error "static assertion failed due to requirement 'ns::S::z<ns::S>'"
//@ diag: 40:87 error "static assertion failed due to requirement 'ns::S::y'"
//@ requires: clang>=22
// clang 22 spelling of static-assert-variable-template.cpp (WP-377).
namespace ns {
template <class T> constexpr bool f = false;
struct S {
  static constexpr bool y = false;
  template <class T> static constexpr bool z = false;
};
template <class...> constexpr bool e = false;
} // namespace ns
template <class T> struct A {
  static constexpr bool y = sizeof(T) == 0;
  template <class U> static constexpr bool v = false;
  void h() { static_assert(A::v<T>); static_assert(A::y); }
};
template void A<int>::h();
static_assert(ns::f<int>);
static_assert(::ns::f<int *>);
static_assert(ns::S::z<char>);
static_assert(A<int>::v<long>);
static_assert(ns::e<int, char>);
template <class T>
void g() { static_assert(ns::S::z<T>); static_assert(T::template z<T>); static_assert(T::y); }
template void g<ns::S>();
