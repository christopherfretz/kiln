// [temp.param]/12, [class.access.general]/7: a default template argument
// of a namespace-scope template that names a private member is checked in
// the template's context, which has no access, wherever the default is
// used: in a friend's member declarations and body, at namespace scope,
// and for a function template as a substitution failure. Found running
// GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:41 error "'type' is a private member of 'C'"
//@ diag: 28:3 note "in instantiation of default argument for 'K<C>' required here"
//@ diag: 24:16 note "declared private here"
//@ diag: 26:41 error "'type' is a private member of 'C'"
//@ diag: 29:13 note "in instantiation of default argument for 'K<C>' required here"
//@ diag: 24:16 note "declared private here"
//@ diag: 26:41 error "'type' is a private member of 'C'"
//@ diag: 30:14 note "in instantiation of default argument for 'K<C>' required here"
//@ diag: 24:16 note "declared private here"
//@ diag: 33:9 error "no matching function for call to 'g'"
//@ diag: 32:51 note "candidate template ignored: substitution failure [with T = C, U = typename C::type]: 'type' is a private member of 'C'"
struct P;
struct C {
  friend struct P;
private:
  typedef void type;
};
template<class T, class U = typename T::type> struct K {};
struct P {
  K<C> *k;
  using T = K<C>;
  void f() { K<C> k; }
};
template<class T, class U = typename T::type> int g() { return 0; }
int x = g<C>();
