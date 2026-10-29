// [expr.dynamic.cast]/6 (clang -fno-rtti): a run-time-checked dynamic_cast
// needs RTTI.
//@ expect: compile-fail
//@ strict-diags
//@ args: -fno-rtti
//@ diag: 10:8 error "use of dynamic_cast requires -frtti"
struct A { virtual void f(); };
struct B : A {};
A* pa; A& ra = *pa; B mkb();
B* x = dynamic_cast<B*>(pa);
