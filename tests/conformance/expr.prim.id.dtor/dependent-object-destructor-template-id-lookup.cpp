// [basic.lookup.qual.general]/2, /4.4, [expr.prim.id.dtor]/2: `~C<args>`
// kept unresolved on a dependent object is looked up in the object's class
// at instantiation; it must name a class template there whose
// specialization is the object's class: `TI` is not a member of
// `N::Inner`, `~TI<long>` is not the destructor of `N::TI<int>`, and `V`
// is not a template (WP-1173).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:39 error "no member named '~TI' in 'N::TI<int>'"
//@ diag: 23:44 note "in instantiation of function template specialization 'e<N::TI<int>>' requested here"
//@ diag: 20:39 error "no member named 'TI' in 'N::Inner'"
//@ diag: 23:50 note "in instantiation of function template specialization 'd<N::Inner>' requested here"
//@ diag: 22:39 error "'V' does not refer to a template"
//@ diag: 23:56 note "in instantiation of function template specialization 'g<N::Inner>' requested here"
//@ diag: 18:22 note "declared as a non-template here"
namespace N {
template <class X> struct TI { ~TI(); };
struct Inner { using V = int; };
}
template <class T> void d(T* p) { p->~TI<int>(); }
template <class T> void e(T* p) { p->~TI<long>(); }
template <class T> void g(T* p) { p->~V<int>(); }
void f(N::TI<int>* p, N::Inner* q) { d(p); e(p); d(q); g(q); }
