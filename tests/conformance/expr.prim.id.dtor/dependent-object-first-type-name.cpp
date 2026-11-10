// [basic.lookup.qual.general]/2, /4.2, [expr.prim.id.dtor]/2, [expr.ref]/4:
// in `p->Self::F::~Q()` on a dependent object, `Self` is a member-qualified
// name looked up in the object's class at instantiation; the first
// type-name `Self::F` must then name a class of which `~Q` is the
// destructor and which is the object's class (or a base of it). Each
// mismatch is diagnosed at instantiation as for a non-dependent object:
// `~Inner` is not a member of `S::Other`, `S::I` is not a class, and
// `S::Other::~Other` does not destroy an `S::Inner`; in `~Self` after
// `Self::Inner::` the name is looked up in the prefix `S` only (4.2), which
// has no `Self` (WP-1173). clang 19 does not look `Self` up in the object's
// class ("use of undeclared identifier 'Self'"), hence clang>=22.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 30:52 error "no member named '~Inner' in 'S::Other'"
//@ diag: 34:30 note "in instantiation of function template specialization 'b<S::Inner>' requested here"
//@ diag: 31:44 error "'Self::I' (aka 'int') is not a class, namespace, or enumeration"
//@ diag: 34:36 note "in instantiation of function template specialization 'c<S::Inner>' requested here"
//@ diag: 32:52 error "'S::Other::~Other' is not a member of class 'S::Inner'"
//@ diag: 34:42 note "in instantiation of function template specialization 'd<S::Inner>' requested here"
//@ diag: 33:52 error "no type named 'Self' in 'S'"
//@ diag: 34:48 note "in instantiation of function template specialization 'e<S::Inner>' requested here"
struct S {
  struct Inner { using Self = S; ~Inner(); };
  struct Other { ~Other(); };
  using I = int;
};
// Valid: `Self` is `S::Inner::Self`, `Self::Inner` is the object's class.
template <class T> void ok(T* p) { p->Self::Inner::~Inner(); }
template <class T> void b(T* p) { p->Self::Other::~Inner(); }
template <class T> void c(T* p) { p->Self::I::~Inner(); }
template <class T> void d(T* p) { p->Self::Other::~Other(); }
template <class T> void e(T* p) { p->Self::Inner::~Self(); }
void f(S::Inner* p) { ok(p); b(p); c(p); d(p); e(p); }
