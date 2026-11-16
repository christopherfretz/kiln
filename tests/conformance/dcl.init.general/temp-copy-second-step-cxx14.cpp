// C++14 [dcl.init]/17.6.3 with [over.best.ics]/4: a copy-initialization by
// user-defined conversion creates a temporary that then direct-initializes
// the object, a copy that may be elided but must be viable, and whose
// argument takes no second user-defined conversion: a copy constructor
// taking a non-const reference cannot bind the temporary, and the
// `auto_ptr`-style constructor from a proxy class is not reached through
// a conversion function to the proxy. Through a converting constructor
// (`B b = 1;`) and through a conversion function (`h(d)`, a template
// conversion as in the old `auto_ptr`). From C++17 the prvalue initializes
// the object directly (the -cxx17 sibling). Found running GCC's C++
// testsuite (WP-1513).
// The error for `return 1;` is at the expression; clang reports it at `return`.
//@ args: -std=c++14
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:3 error "no viable constructor copying variable of type 'B'"
//@ diag: 40:12 note "candidate constructor not viable: no known conversion from 'B' to 'int' for 1st argument"
//@ diag: 40:20 note "candidate constructor not viable: expects an lvalue for 1st argument"
//@ diag: 40:27 note "candidate constructor not viable: no known conversion from 'B' to 'R' for 1st argument"
//@ diag: 43:15 error "no viable constructor copying parameter of type 'B'"
//@ diag: 40:12 note "candidate constructor not viable: no known conversion from 'B' to 'int' for 1st argument"
//@ diag: 40:20 note "candidate constructor not viable: expects an lvalue for 1st argument"
//@ diag: 40:27 note "candidate constructor not viable: no known conversion from 'B' to 'R' for 1st argument"
//@ diag: 44 error "no viable constructor returning object of type 'B'"
//@ diag: 40:12 note "candidate constructor not viable: no known conversion from 'B' to 'int' for 1st argument"
//@ diag: 40:20 note "candidate constructor not viable: expects an lvalue for 1st argument"
//@ diag: 40:27 note "candidate constructor not viable: no known conversion from 'B' to 'R' for 1st argument"
//@ diag: 46:18 error "no viable constructor copying parameter of type 'B'"
//@ diag: 40:12 note "candidate constructor not viable: no known conversion from 'B' to 'int' for 1st argument"
//@ diag: 40:20 note "candidate constructor not viable: expects an lvalue for 1st argument"
//@ diag: 40:27 note "candidate constructor not viable: no known conversion from 'B' to 'R' for 1st argument"
//@ diag: 46:24 error "no viable constructor copying variable of type 'B'"
//@ diag: 40:12 note "candidate constructor not viable: no known conversion from 'B' to 'int' for 1st argument"
//@ diag: 40:20 note "candidate constructor not viable: expects an lvalue for 1st argument"
//@ diag: 40:27 note "candidate constructor not viable: no known conversion from 'B' to 'R' for 1st argument"
//@ diag: 59:16 error "no viable constructor copying parameter of type 'P<Base>'"
//@ diag: 50:3 note "candidate constructor not viable: expects an lvalue for 1st argument"
//@ diag: 51:3 note "candidate constructor not viable: no known conversion from 'P<Base>' to 'Ref<Base>' for 1st argument"
struct R { R(int); };
struct B { B(int); B(B&); B(R); };
B b = 1;
void h(B);
void t1() { h(1); }
B t2() { return 1; }
struct D { operator B(); };
void t3(D d) { h(d); B x = d; (void)x; }

template <class Y> struct Ref { Ref(Y*); };
template <class X> struct P {
  P(P&);
  P(Ref<X>);
  template <class Y> operator Ref<Y>();
  template <class Y> operator P<Y>();
};
struct Base {};
struct Der : Base {};
P<Der> f();
void hb(P<Base>);
void t4() { hb(f()); }
