// [temp.dep.type]/6, [temp.res.general]/6: without a dependent base, a
// qualified name in the current instantiation that finds nothing is
// diagnosed at definition; with one, at instantiation (WP-309).
//@ expect: compile-fail
//@ strict-diags
// Verified against clang 19 (WP-309), except that clang names the first
// class 'V<T>'.
//@ diag: 11:51 error "no member named 'nope' in 'V"
//@ diag: 12:55 error "no member named 'nope' in 'W<E>'"
//@ diag: 14:25 note "in instantiation of member function 'W<E>::f' requested here"
template <class T> struct V { int f() { return V::nope; } };
template <class T> struct W : T { int f() { return W::nope; } };
struct E {};
int g() { return W<E>().f(); }
