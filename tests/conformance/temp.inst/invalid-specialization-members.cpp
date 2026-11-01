// [temp.inst]/2-4: the use that instantiates a class template specialization whose instantiation
// fails is itself erroneous with no further diagnostic (clang's `RequireCompleteType` returning
// the instantiation's invalidity): a variable or parameter of that type is invalid, and a member
// access through which it was instantiated builds nothing, so the member functions named there
// are not instantiated. Later uses find the class complete: a static member, a member pointer, or
// a second member call instantiate their member as usual.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:31 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 36:8 note "in instantiation of template class 'A<int>' requested here"
//@ diag: 39:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 43:8 note "in instantiation of template class 'B<int>' requested here"
//@ diag: 46:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 51:8 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 54:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 58:26 note "in instantiation of template class 'D<int>' requested here"
//@ diag: 59:40 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 60:19 note "in instantiation of template class 'E<int>' requested here"
//@ diag: 62:12 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 70:18 note "in instantiation of template class 'F<int>' requested here"
//@ diag: 72:6 error "cannot initialize a variable of type 'int *' with an lvalue of type 'int'"
//@ diag: 73:16 error "no member named 'nope' in 'F<int>'"
//@ diag: 33:21 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 37:31 note "in instantiation of member function 'A<int>::h' requested here"
//@ diag: 64:15 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 70:29 note "in instantiation of member function 'F<int>::g2' requested here"
//@ diag: 65:22 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 70:39 note "in instantiation of member function 'F<int>::v' requested here"
//@ diag: 66:14 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 71:33 note "in instantiation of member function 'F<int>::k' requested here"
template <class T> struct A : T::base {
  void g() { T::n1(); }
  static void h() { T::n2(); }
  A() {}
};
A<int> a;
void use_a() { a.g(); A<int>::h(); }
template <class T> struct B {
  typename T::type m;
  void g() { T::n3(); }
  int y = T::z;
};
B<int> b;
int kb = (b.g(), b.y);
template <class T> struct C {
  typename T::type m;
  C(int) {}
  void g() { T::n4(); }
  ~C() { T::n5(); }
};
C<int> c(1);
void use_c() { c.g(); }
template <class T> struct D {
  typename T::type m;
  void g();
};
template <class T> void D<T>::g() { T::n6(); }
void use_d(D<int> &d) { d.g(); }
template <class T> struct E { typename T::type m; void g() { T::n7(); } };
void use_e(E<int> e) { e.g(); }
template <class T> struct F {
  typename T::type m;
  void g() { T::n8(); }
  void g2() { T::n9(); }
  virtual void v() { T::n10(); }
  void k() { T::n11(); }
  int d;
};
F<int> *fp;
void use_f() { fp->g(); fp->g2(); fp->v(); }
void (F<int>::*pm)() = &F<int>::k;
int *w = fp->d;
int nope = fp->nope;
int main() {}
