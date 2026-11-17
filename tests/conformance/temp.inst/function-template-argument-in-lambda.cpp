// [basic.def.odr]/3, [temp.inst]/11: a function template specialization
// named as a non-type template argument in the body of a non-generic
// lambda inside a template is odr-used and instantiated with each
// instantiation of the enclosing definition (the lambda's body is
// instantiated with it), as at namespace scope; also in a nested lambda
// and in a member function of a class template. clang 22 instantiates
// none of them, GCC 16 the first two (`g`); the text decides (WP-1591).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(clang): clang 22 does not instantiate a specialization named as a template argument in a template body; the text and GCC 16 do
//@ diag: 24:32 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 29:19 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 37:14 note "in instantiation of function template specialization 'g<0>' requested here"
//@ diag: 24:32 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 29:31 note "in instantiation of function template specialization 'f<char>' requested here"
//@ diag: 37:14 note "in instantiation of function template specialization 'g<0>' requested here"
//@ diag: 24:32 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 32:31 note "in instantiation of function template specialization 'f<long>' requested here"
//@ diag: 37:22 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 24:32 error "type 'short' cannot be used prior to '::' because it has no members"
//@ diag: 35:30 note "in instantiation of function template specialization 'f<short>' requested here"
//@ diag: 37:41 note "in instantiation of member function 'D<int>::k' requested here"
template <class T> void f(T) { T::fail; }
template <void (*P)(int)> struct A {};
template <void (*P)(char)> void wrap() {}
template <auto P> struct B {};
template <int> void g() {
  auto l = [] { A<&f> a; wrap<&f>(); };
}
template <class> void h() {
  auto l = [] { return [] { B<&f<long>> b; }; };
}
template <class U> struct D {
  void k() { auto l = [] { B<&f<short>> b; }; }
};
void use() { g<0>(); h<int>(); D<int>().k(); }
