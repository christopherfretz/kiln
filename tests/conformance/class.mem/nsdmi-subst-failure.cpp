// [class.mem.general]/8, [class.base.init]/9.1, [temp.inst]/3, [except.spec]/7:
// a default member initializer of a class template specialization is
// instantiated when first needed (clang `InstantiateInClassInitializer`): by the
// implicit exception specification of the default constructor
// ("in evaluation of exception specification ... needed here", after the
// initializer's own note), by a constructor that uses it (requested at the
// constructor, an out-of-line one at its definition), or by aggregate
// initialization. A failed one is reported once; the field is invalid after.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:14 error "no member named 'nope' in 'X'"
//@ diag: 26:27 note "in instantiation of default member initializer 'A<X>::v' requested here"
//@ diag: 35:6 note "in evaluation of exception specification for 'A<X>::A' needed here"
//@ diag: 50:42 error "no member named 'nope' in 'X'"
//@ diag: 50:48 note "in instantiation of default member initializer 'F<X>::v' requested here"
//@ diag: 51:6 note "in evaluation of exception specification for 'F<X>::F' needed here"
//@ diag: 52:53 error "no member named 'nope' in 'X'"
//@ diag: 53:8 note "in instantiation of default member initializer 'G<X>::w' requested here"
//@ diag: 31:14 error "no member named 'nope' in 'X'"
//@ diag: 32:3 note "in instantiation of default member initializer 'C<X>::v' requested here"
//@ diag: 36:6 note "in instantiation of member function 'C<X>::C' requested here"
//@ diag: 46:42 error "no member named 'nope' in 'X'"
//@ diag: 47:26 note "in instantiation of default member initializer 'E<X>::v' requested here"
//@ diag: 48:6 note "in instantiation of member function 'E<X>::E' requested here"
struct X {};
template <class T> struct A {
  int v = T::nope;
  int w = 1;
};
template <class T> struct C {
  int v = T::nope;
  C() {}
  C(int) : v(0) {}
};
A<X> a;
C<X> c;
C<X> c2(1);
int uses() { return a.w + c.v + c2.v; }
A<X> make() { return {}; }
C<X> copy = c;
// Naming the class or taking its size needs no initializer.
template <class T> struct D { int v = T::nope; };
D<X> *dp;
int sz = sizeof(D<X>);
// An out-of-line constructor requests it at its definition.
template <class T> struct E { int v = T::nope; E(); };
template <class T> E<T>::E() {}
E<X> e;
// A defaulted one, and aggregate initialization.
template <class T> struct F { int v = T::nope; F() = default; };
F<X> f;
template <class T> struct G { int v = 0; int w = T::nope; };
G<X> g{};
G<X> g2{1, 2};
