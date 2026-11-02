// [dcl.enum]/5, [meta.trans.other] (`underlying_type` Mandates): an
// enumeration without a fixed underlying type is incomplete until its `}`,
// so `__underlying_type(E)` in an enumerator is an error with a note at the
// forward declaration, also when a class template instantiation forms it.
// A fixed or scoped enumeration has its underlying type at once.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:23 error "cannot determine underlying type of incomplete enumeration type 'E1'"
//@ diag: 24:6 note "forward declaration of 'E1'"
//@ diag: 20 error "cannot determine underlying type of incomplete enumeration type 'E2'"
//@ diag: 22:32 note "in instantiation of template class 'impl<E2>' requested here"
//@ diag: 25:23 note "in instantiation of template class 'ut<E2>' requested here"
//@ diag: 25:6 note "forward declaration of 'E2'"
//@ diag: 20 error "cannot determine underlying type of incomplete enumeration type 'E3'"
//@ diag: 26:23 note "in instantiation of template class 'impl<E3>' requested here"
//@ diag: 26:6 note "forward declaration of 'E3'"
template <class T, bool = __is_enum(T)> struct impl;
template <class T> struct impl<T, false> {};
template <class T> struct impl<T, true> {
  typedef __underlying_type(T) type;
};
template <class T> struct ut : impl<T> {};
template <class T> using ut_t = typename ut<T>::type;
enum E1 { a1 = sizeof(__underlying_type(E1)) };
enum E2 { a2 = sizeof(ut<E2>::type) };
enum E3 { a3 = sizeof(impl<E3>::type) };
enum E4 : char { a4 = sizeof(ut<E4>::type) };
enum class E5 { a5 = sizeof(ut<E5>::type) };
static_assert(sizeof(ut_t<E4>) == 1);
