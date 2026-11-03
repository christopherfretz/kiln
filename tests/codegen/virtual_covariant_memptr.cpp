// Covariant returns through pointers to member functions ([class.virtual]/8,
// [expr.mptr.oper]/6): a member pointer names a vtable slot, and the slot it
// names fixes the adjustment. `&Base::get` uses Derived's primary-base slot,
// whose entry is the return-adjusting thunk (_ZTch, B* -> A* at a non-zero
// offset); `&Derived::get` uses the slot Derived adds for its own return
// type, whose entry is the overrider itself (no thunk, no adjustment).
struct A { int a = 1; virtual ~A() noexcept {} };
struct X { int x = 2; virtual ~X() noexcept {} };
struct B : X, A { int b = 3; };
struct Base {
  virtual A *get() noexcept { return nullptr; }
  virtual ~Base() noexcept {}
};
struct Derived : Base {
  B bb;
  B *get() noexcept override { return &bb; }
};
using BaseMp = A *(Base::*)() noexcept;
using DerMp = B *(Derived::*)() noexcept;
A *call_base(Base &o, BaseMp mp) noexcept { return (o.*mp)(); }
B *call_der(Derived &o, DerMp mp) noexcept { return (o.*mp)(); }
A *via_base(Derived &d) noexcept { return call_base(d, &Base::get); }
B *via_der(Derived &d) noexcept { return call_der(d, &Derived::get); }
int main() noexcept { Derived d; return via_base(d)->a + via_der(d)->b; }
