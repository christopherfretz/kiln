//@ expect: vtables
struct A { virtual void f(); int a; };
struct B { virtual void f(); int b; };
struct C : A, B { void f(); };
void A::f() {} void B::f() {} void C::f() {}
