// [dcl.type.elab]/1, [class.friend]/3: `friend class T::template B<int>;`
// befriends the specialization the instantiation names, and no other
// class; with `B` no template in the argument, the instantiation is
// ill-formed ([temp.names]/6) (WP-1495).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 16:56 error "'secret' is a private member of 'F<G>'"
//@ diag: 13:66 note "implicitly declared private here"
//@ diag: 13:54 error "'B' following the 'template' keyword does not refer to a template"
//@ diag: 18:6 note "in instantiation of template class 'F<K>' requested here"
//@ diag: 17:19 note "declared as a non-template here"
template<class T> class F { friend class T::template B<int>; int secret = 1; };
struct G { template<class> struct B { int get(F<G> &f) { return f.secret; } }; };
int use(F<G> &f) { return G::B<int>().get(f); }
template<class> struct H { int get(F<G> &f) { return f.secret; } };
struct K { struct B {}; };
F<K> fk;
