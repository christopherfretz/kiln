// [class.access]/7: the names in a declaration (and its template head) are
// checked in the context of the declared entity: an out-of-line member, a
// befriended function or class template; anything else is checked outside A.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:4 error "'I' is a private member of 'A'"
//@ diag: 9:15 note "implicitly declared private here"
class A {
  typedef int I;
  static I x;
  template<int> struct Q;
  template<int> friend struct R;
  I f();
  friend I g(I);
  struct C { struct In { }; void m(); };
};
A::I A::f() { return 0; }
A::I g(A::I p);
A::I g(A::I p) { return p; }
A::I A::x = 0;
template<A::I> struct A::Q { A::I q; };
template<A::I> struct R { A::I r; };
void A::C::m() { A::C::In in; (void)in; }
A::I h();
