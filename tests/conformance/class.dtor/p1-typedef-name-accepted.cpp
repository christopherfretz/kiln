// [class.dtor]/1: -Wno-dtor-typedef accepts a typedef naming the class in a
// destructor declaration; a typedef in a pseudo-destructor call or after
// the qualifier names the class and is always accepted.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-dtor-typedef
struct A { typedef A A2; ~A2(); };
A::~A() {}
struct C { ~C(); }; typedef C C2;
C::~C2() {}
void f(C *p) { p->~C2(); }
