// [temp.inst]/4, [basic.def.odr]/1: two class templates defining the same
// friend function are not a redefinition until both are instantiated; the
// error is reported at the second instantiation, also against an ordinary
// earlier definition. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:43 error "redefinition of 'g'"
//@ diag: 19:18 note "in instantiation of template class 'B<int>' requested here"
//@ diag: 13:43 note "previous definition is here"
//@ diag: 18:43 error "redefinition of 'k'"
//@ diag: 20:8 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 17:6 note "previous definition is here"
template <class T> struct A { friend void g(int) {} };
template <class T> struct B { friend void g(int) {} };
template <class T> struct Y { friend void y(int) {} };
Y<int> y0;
void k(int) {}
template <class T> struct C { friend void k(int) {} };
A<int> a; B<int> b;
C<int> c;
