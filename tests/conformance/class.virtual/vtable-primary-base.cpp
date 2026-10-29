// [class.virtual]/2 (final overriders), Itanium C++ ABI §2.5.2 (the primary
// base's vtable is a prefix of the derived class's), §2.5.3 (secondary
// vtables), §2.6 (non-virtual this-adjusting thunks for overriders reached
// through a secondary base).
//@ expect: vtables
struct A { virtual void f(); virtual void g(); int a; };
struct B { virtual void f(); virtual void h(); int b; };
struct C : A, B {
  void f() override;                      // overrides A::f and B::f (thunk)
  virtual void k();
};
void A::f() {} void A::g() {} void B::f() {} void B::h() {}
void C::f() {} void C::k() {}
