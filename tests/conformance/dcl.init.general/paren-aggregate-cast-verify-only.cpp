// [dcl.init.general]/16.6.2.2, [expr.type.conv]/2, [expr.static.cast]/4,
// [expr.cast]/4: a cast to an aggregate class with no viable constructor
// tries parenthesized aggregate initialization verify-only (clang
// `TryOrBuildParenListInitialization` with `VerifyOnly`). When it fails
// (here: the reference member `r` has no initializer) the cast reports the
// constructor overload failure, and the attempt leaves no trace: the
// converting constructor template it selected for `c` is not odr-used, so
// its static_assert is never instantiated. WP-328: kiln instantiated it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:9 error "no matching conversion for functional-style cast from 'int' to 'A'"
//@ diag: 26:8 note "candidate constructor (the implicit copy constructor) not viable"
//@ diag: 26:8 note "candidate constructor (the implicit move constructor) not viable"
//@ diag: 26:8 note "candidate constructor (the implicit default constructor) not viable"
//@ diag: 29:9 error "no matching conversion for C-style cast from 'float' to 'A'"
//@ diag: 26:8 note "candidate constructor (the implicit copy constructor) not viable"
//@ diag: 26:8 note "candidate constructor (the implicit move constructor) not viable"
//@ diag: 26:8 note "candidate constructor (the implicit default constructor) not viable"
//@ diag: 30:9 error "no matching conversion for static_cast from 'long' to 'A'"
//@ diag: 26:8 note "candidate constructor (the implicit copy constructor) not viable"
//@ diag: 26:8 note "candidate constructor (the implicit move constructor) not viable"
//@ diag: 26:8 note "candidate constructor (the implicit default constructor) not viable"
struct C {
  template <class V> C(V) { static_assert(sizeof(V) == 0, "instantiated"); }
};
struct A { C c; int &r; };
void g() {
  (void)A(1);
  (void)(A)1.0f;
  (void)static_cast<A>(1L);
}
