// [temp.spec.general]/6, [temp.deduct.general]/8: names written in an
// explicit instantiation are not access checked, but the substitution it
// triggers (deduction, default template arguments, the substituted
// signature) is checked in the template's context, an inaccessible name a
// substitution failure. A SFINAE substitution inside an ordinary
// declaration's declarator is not delayed to the declaration either.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:14 error "no matching function for call to 'dt'"
//@ diag: 27:49 note "candidate template ignored: substitution failure [with T = C, U = typename C::P]: 'P' is a private member of 'C'"
//@ diag: 29:15 error "explicit instantiation of 'dt' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 27:49 note "candidate template ignored: substitution failure [with T = C, U = typename C::P]: 'P' is a private member of 'C'"
//@ diag: 30:15 error "explicit instantiation of 'dt' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 27:49 note "candidate template ignored: substitution failure [with T = C, U = typename C::P]: 'P' is a private member of 'C'"
//@ diag: 32:15 error "explicit instantiation of 'dn' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 31:40 note "candidate template ignored: substitution failure [with T = C]: 'obj' is a private member of 'C'"
//@ diag: 34:15 error "explicit instantiation of 'rt' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 33:24 note "candidate template ignored: substitution failure [with T = C]: 'P' is a private member of 'C'"
//@ diag: 36:23 error "explicit instantiation of 'm' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 35:78 note "candidate template ignored: substitution failure [with U = C, V = typename C::P]: 'P' is a private member of 'C'"
//@ requires: clang<22
// clang 21+ does not access check the deduction of explicit-instantiation candidates: access-in-substitution.cpp.
class C {
  typedef int P;
  static int obj;
};
template<class T, class U = typename T::P> void dt(T) {}
void use() { dt(C()); }
template void dt<C>(C);
template void dt(C);
template<class T, int N = T::obj> void dn(T) {}
template void dn(C);
template<class T> auto rt(T) -> typename T::P { return 0; }
template auto rt(C) -> int;
template<class T> struct D { template<class U, class V = typename U::P> void m(U) {} };
template void D<int>::m(C);
template<class T> T ok(T t) { return t; }
template C::P ok<C::P>(C::P);
template<class T, class U = typename T::P> int sf(T) { return 0; }
long sf(...);
decltype(sf(C())) y;
static_assert(sizeof(y) == sizeof(long));
