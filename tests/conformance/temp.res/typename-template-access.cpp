// [class.access]/4, [temp.res.general], [temp.deduct.general]/8: a private member
// class template named by `typename T::template Z<int>` is diagnosed at the `Z` of
// each instantiated declaration (a member function's return type, a data member, a
// parameter, a nested class's member), and is a substitution failure in deduction.
// A friend's access is checked from the friend. As are `T::type` in a return type
// and in a default argument's `sizeof`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 40:24 error "'Z' is a private member of 'P'"
//@ diag: 49:6 note "in instantiation of template class 'A<P>' requested here"
//@ diag: 31:29 note "implicitly declared private here"
//@ diag: 41:24 error "'Z' is a private member of 'P'"
//@ diag: 31:29 note "implicitly declared private here"
//@ diag: 42:31 error "'Z' is a private member of 'P'"
//@ diag: 31:29 note "implicitly declared private here"
//@ diag: 43:31 error "'Z' is a private member of 'P'"
//@ diag: 31:29 note "implicitly declared private here"
//@ diag: 44:15 error "'type' is a private member of 'P'"
//@ diag: 32:15 note "implicitly declared private here"
//@ diag: 46:26 error "'Z' is a private member of 'P'"
//@ diag: 50:9 note "in instantiation of member class 'A<P>::N' requested here"
//@ diag: 31:29 note "implicitly declared private here"
//@ diag: 44:47 error "'type' is a private member of 'P'"
//@ diag: 51:10 note "in instantiation of default function argument expression for 'g<P>' required here"
//@ diag: 32:15 note "implicitly declared private here"
//@ diag: 54:10 error "no matching function for call to 'k'"
//@ diag: 53:48 note "candidate template ignored: substitution failure [with T = P]: 'Z' is a private member of 'P'"
//@ requires: clang<22
// clang 19 spelling; typename-template-access-clang22.cpp has clang 22's (WP-394).
class P {
  template <class U> struct Z {};
  typedef int type;
};
class Q {
  template <class U> struct Z {};
  friend struct F;
};

template <class T> struct A {
  typename T::template Z<int> z();
  typename T::template Z<int> zz;
  static typename T::template Z<int> s;
  void f(typename T::template Z<int>);
  typename T::type g(int = sizeof(typename T::type));
  struct N {
    typename T::template Z<int> n();
  };
};
A<P> a;
A<P>::N n;
int da = a.g();

template <class T> typename T::template Z<int> k() { return {}; }
auto x = k<P>();

struct F {
  template <class T> static typename T::template Z<int> ok() { return {}; }
};
auto y = F::ok<Q>();
