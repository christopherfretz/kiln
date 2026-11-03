// [temp.spec.general]/6, [temp.deduct.general]/8: names written in an
// explicit instantiation or specialization are not access checked. clang
// 21+ also deduces their candidates with access checks suppressed (rt,
// pt, D<int>::m, dt, es are found); a candidate that fails for another
// reason shows the first access error as its reason (dn: T::obj is not a
// constant), and constraint satisfaction still checks access (g). A
// SFINAE substitution elsewhere is checked (dc, sf).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 20:14 error "no matching function for call to 'dc'"
//@ diag: 19:49 note "candidate template ignored: substitution failure [with T = C, U = typename C::P]: 'P' is a private member of 'C'"
//@ diag: 26:15 error "explicit instantiation of 'dn' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 25:40 note "candidate template ignored: substitution failure [with T = C]: 'obj' is a private member of 'C'"
//@ diag: 34:15 error "explicit instantiation of 'g' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 33:24 note "candidate template ignored: constraints not satisfied [with T = C]"
//@ diag: 33:42 note "because substituted constraint expression is ill-formed: 'k' is a private member of 'C'"
class C { typedef int P; static int obj; static const int k = 1; };
template<class T, class U = typename T::P> void dc(T) {}
void use() { dc(C()); }
template<class T, class U = typename T::P> void dt(T) {}
template void dt<C>(C);
template<class T, int N = T::k> void dk(T) {}
template void dk(C);
template<class T, int N = T::obj> void dn(T) {}
template void dn(C);
template<class T> auto rt(T) -> typename T::P { return 0; }
template auto rt(C) -> int;
template<class T> void pt(T, decltype(T::obj)* = 0) {}
template void pt(C, int*);
template<class T> struct D { template<class U, class V = typename U::P> void m(U) {} };
template void D<int>::m(C);
template<class T> void g(T) requires (T::k == 1) {}
template void g(C);
template<class T, class U = typename T::P> void es(T) {}
template<> void es(C) {}
template<class T> T ok(T t) { return t; }
template C::P ok<C::P>(C::P);
template<class T, class U = typename T::P> int sf(T) { return 0; }
long sf(...);
decltype(sf(C())) y;
static_assert(sizeof(y) == sizeof(long));
