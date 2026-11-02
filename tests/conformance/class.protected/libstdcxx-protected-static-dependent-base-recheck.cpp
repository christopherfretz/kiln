// [class.protected]/1, [class.access.base]/5, [temp.dep.type]: in a class
// template with a dependent base, whether the class derives from the class
// naming a protected member is known only once instantiated, so the access
// check is made per instantiation: `user<facet>` may name facet::c_name,
// `user<other>` may not (companion of
// libstdcxx-protected-static-dependent-base.cpp, WP-323).
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:27 error "'c_name' is a protected member of 'facet'"
//@ diag: 23:23 note "in instantiation of member function 'user<other>::f' requested here"
//@ diag: 15:14 note "declared protected here"
struct facet {
protected:
  static int c_name() { return 0; }
};
struct other {};
template<class T> struct mid : T {};
template<class T> struct user : mid<T> {
  int f() { return facet::c_name(); }
};
int a = user<facet>().f();
int b = user<other>().f();
