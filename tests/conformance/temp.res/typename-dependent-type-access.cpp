// [temp.res.general]/3, [class.access]: a missing or private member named
// by `typename T::type` is diagnosed at the written `T` of each
// instantiated declaration and function signature.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:15 error "no type named 'type' in 'B'"
//@ diag: 30:6 note "in instantiation of template class 'A<B>' requested here"
//@ diag: 24:22 error "no type named 'type' in 'B'"
//@ diag: 25:15 error "no type named 'type' in 'B'"
//@ diag: 26:22 error "no type named 'type' in 'B'"
//@ diag: 23:15 error "'type' is a private member of 'C'"
//@ diag: 31:6 note "in instantiation of template class 'A<C>' requested here"
//@ diag: 29:23 note "implicitly declared private here"
//@ diag: 24:22 error "'type' is a private member of 'C'"
//@ diag: 29:23 note "implicitly declared private here"
//@ diag: 25:15 error "'type' is a private member of 'C'"
//@ diag: 29:23 note "implicitly declared private here"
//@ diag: 26:22 error "'type' is a private member of 'C'"
//@ diag: 29:23 note "implicitly declared private here"
//@ diag: 34:9 error "no matching function for call to 'k'"
//@ diag: 33:36 note "candidate template ignored: substitution failure [with T = C]: 'type' is a private member of 'C'"
template<class T> struct A {
  typename T::type x;
  static typename T::type y;
  typename T::type f();
  void g(typename T::type);
};
struct B { };
class C { typedef int type; };
A<B> a;
A<C> c;
template<class T> void h(typename T::type p) {}
template<class T> typename T::type k() { return 0; }
int z = k<C>();
