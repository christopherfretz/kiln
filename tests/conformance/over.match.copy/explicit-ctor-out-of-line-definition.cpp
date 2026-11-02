// [over.match.copy]/1.1, [dcl.fct.spec]/4: an explicit constructor or
// conversion function defined out of line stays explicit (the
// explicit-specifier belongs to the declaration in the class), so
// copy-initialization does not consider it; direct-initialization does. The
// note points at the declaration that carries `explicit`. A member template
// of a class template (`complex<float>(const complex<double>&)`) too.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:5 error "no viable conversion from 'X' to 'Y'"
//@ diag: 21:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'X' to 'const Y &' for 1st argument"
//@ diag: 21:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'X' to 'Y &&' for 1st argument"
//@ diag: 21:21 note "explicit constructor is not a candidate"
//@ diag: 33:7 error "no viable conversion from 'X' to 'int'"
//@ diag: 22:28 note "explicit conversion function is not a candidate"
//@ diag: 37:12 error "no viable conversion from 'C<double>' to 'C<float>'"
//@ diag: 25:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'C<double>' to 'const C<float> &' for 1st argument"
//@ diag: 25:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'C<double>' to 'C<float> &&' for 1st argument"
//@ diag: 26:3 note "candidate constructor not viable: no known conversion from 'C<double>' to 'float' for 1st argument"
//@ diag: 27:31 note "explicit constructor is not a candidate"
struct X;
struct Y { explicit Y(const X&); Y(); };
struct X { int a; explicit operator int() const; };
Y::Y(const X&) {}
X::operator int() const { return a; }
template <class T> struct C {
  C(T) {}
  template <class U> explicit C(const C<U>&);
};
template <class T> template <class U> C<T>::C(const C<U>&) {}
void f() {
  X x;
  Y y = x;
  int i = x;
  Y y2(x);
  int j(x);
  C<double> cd(1.0);
  C<float> cf = cd;
  C<float> cf2(cd);
  (void)y; (void)i; (void)y2; (void)j; (void)cf; (void)cf2;
}
