// [basic.def.odr]/8, [temp.inst]/11 (WP-137): a constructor or destructor
// definition, or the definition of a class's key function, uses its vtable,
// which odr-uses every non-pure virtual function in it: a class template's
// virtual members are instantiated (at the end of the TU, the note at the
// use), including through virtual bases (the VTT) and, for a local class,
// at once.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:59 error "no member named 'nl' in 'X'"
//@ diag: 33:23 note "in instantiation of member function 'L<X>::l' requested here"
//@ diag: 25:59 error "no member named 'nf' in 'X'"
//@ diag: 29:4 note "in instantiation of member function 'B<X>::f' requested here"
//@ diag: 26:59 error "no member named 'nv' in 'X'"
//@ diag: 36:5 note "in instantiation of member function 'V<X>::v' requested here"
//@ diag: 27:74 error "no member named 'nm' in 'X'"
//@ diag: 36:5 note "in instantiation of member function 'M<X>::m' requested here"
//@ diag: 25:59 error "no member named 'nf' in 'Y'"
//@ diag: 39:9 note "in instantiation of member function 'B<Y>::f' requested here"
//@ diag: 25:59 error "no member named 'nf' in 'Z'"
//@ diag: 41:22 note "in instantiation of member function 'B<Z>::f' requested here"
struct X {};
struct Y {};
struct Z {};
struct W {};
template <class T> struct B { virtual int f() { return T::nf; } };
template <class T> struct V { virtual int v() { return T::nv; } };
template <class T> struct M : virtual V<T> { virtual int m() { return T::nm; } };
struct D : B<X> { D(); };
D::D() {}
struct E : B<W> { virtual ~E(); };
template <class T> struct L { virtual int l() { return T::nl; } };
void g() {
  struct Loc : L<X> { Loc() {} };
}
struct DV : M<X> { DV(); };
DV::DV()
{}
struct F : B<Y> { virtual void k(); };
void F::k() {}
struct G : B<Z> {};
B<Z> make() { return B<Z>(); }
