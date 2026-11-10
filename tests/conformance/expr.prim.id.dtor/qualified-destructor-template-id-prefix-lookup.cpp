// [basic.lookup.qual.general]/4.2, /4.5, [expr.prim.id.dtor]/1: in
// `p->S::C<int>::~Q<int>()` the nested-name-specifier `S::C<int>::` has a
// prefix `S`, so the template-name `Q` is looked up in `S` only, not in
// the object's class nor where the expression is; an alias template `CA`
// declared at namespace scope outside `S` is not found ("no template named
// 'CA' in 'N::C<int>'", clang's typo correction, which goes on with `CA`).
// GCC 16 accepts the non-dependent forms (WP-1173).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:6 error "no template named 'CA' in 'N::C<int>'; did you mean simply 'CA'?"
//@ diag: 18:20 note "'CA' declared here"
//@ diag: 22:6 error "no template named 'CA' in 'N::C<int>'; did you mean simply 'CA'?"
//@ diag: 18:20 note "'CA' declared here"
//@ diag: 23:5 error "no template named 'D' in 'N::C<int>'; did you mean simply 'D'?"
//@ diag: 19:27 note "'D' declared here"
//@ diag: 23:16 error "no member named '~D' in 'N::C<int>'"
namespace N { template <class X> struct C { ~C(); }; }
template <class X> using CA = N::C<X>;
template <class X> struct D { ~D(); };
void f(N::C<int>* p, N::C<int> c) {
  p->N::C<int>::~CA<int>();
  p->::N::C<int>::~CA<int>();
  c.N::C<int>::~D<int>();
}
