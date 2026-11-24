// [temp.inst], [temp.res.general]: a member type named through a
// substituted template parameter (`typename T::type` with `T = ns::S`)
// prints its qualifier as the canonical replacement, fully scoped with
// clang 22 (`'typename ns::S::type *'`, `ns::W<ns::S>::`,
// `(anonymous namespace)::A::`); clang 19 suppresses the scope (the
// `-19` sibling). WP-3402.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 19:39 error "cannot initialize a variable of type 'int *' with an rvalue of type 'typename ns::S::type *' (aka 'long *')"
//@ diag: 21:24 note "in instantiation of member function 'X<ns::S>::f' requested here"
//@ diag: 19:39 error "cannot initialize a variable of type 'int *' with an rvalue of type 'typename ns::W<ns::S>::type *' (aka 'ns::S *')"
//@ diag: 22:31 note "in instantiation of member function 'X<ns::W<ns::S>>::f' requested here"
//@ diag: 19:39 error "cannot initialize a variable of type 'int *' with an rvalue of type 'typename (anonymous namespace)::A::type *' (aka 'short *')"
//@ diag: 23:20 note "in instantiation of member function 'X<(anonymous namespace)::A>::f' requested here"
namespace ns { struct S { using type = long; }; template <class U> struct W { using type = U; }; }
namespace { struct A { using type = short; }; }
template <class T> struct X {
  typename T::type v; void f() { int *p = &v; (void)p; }
};
void u1() { X<ns::S>().f(); }
void u2() { X<ns::W<ns::S>>().f(); }
void u3() { X<A>().f(); }
