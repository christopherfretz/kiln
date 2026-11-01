// [class.mfct.non.static]/2, [over.match.call]/2.2: an overloaded `B::f`
// named in a member of a class template with a dependent base stays an
// implicit member access; when `B` turns out not to be a base there is no
// object for the call (clang's `diagnoseInstanceReference`, not the
// "is not a member" of a lone `B::h`). An overload set of static members
// needs no object. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:23 error "call to non-static member function without an object argument"
//@ diag: 26:31 note "in instantiation of member function 'D<O>::g' requested here"
//@ diag: 22:24 error "call to non-static member function without an object argument"
//@ diag: 26:39 note "in instantiation of member function 'D<O>::g2' requested here"
//@ diag: 24:24 error "call to non-static member function without an object argument"
//@ diag: 26:57 note "in instantiation of member function 'D<O>::g4' requested here"
struct B {
  int f(); int f(int); static int s(); static int s(int);
  template <class U> int t(U);
};
struct O {};
template <class T> struct D : T {
  int g() { return B::f(); }
  int g2() { return B::f(1); }
  int g3() { return B::s(); }
  int g4() { return B::t(1); }
};
int main() { D<O> d; return d.g() + d.g2() + d.g3() + d.g4(); }
