// [class.virtual]/8, [expr.mptr.oper]/6, [expr.const]: a virtual function
// with a covariant return type called through a pointer to member function
// returns its result converted to the return type of the function the
// pointer names, as a direct virtual call does: through `&Base::get` the
// final overrider's `Derived *` becomes a `Base *` (here at a non-zero
// offset), through `&Derived::get` it is not adjusted. Pointers and
// references, `.*` and `->*`, and a non-virtual member (no adjustment).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
struct Pad { int pad = 4; constexpr virtual int p() const { return pad; } };
struct Base {
  int b = 1;
  constexpr virtual Base *get() { return this; }
  constexpr virtual Base &ref() { return *this; }
  constexpr Base *plain() { return this; }
  constexpr virtual ~Base() = default;
};
struct Derived : Pad, Base {
  int d = 2;
  constexpr Derived *get() override { return this; }
  constexpr Derived &ref() override { return *this; }
};

constexpr int f() {
  Derived obj;
  Base *bp = &obj;
  Base *(Base::*bg)() = &Base::get;
  Base &(Base::*br)() = &Base::ref;
  Derived *(Derived::*dg)() = &Derived::get;
  Base *(Base::*bn)() = &Base::plain;
  Base *x = (bp->*bg)();
  Base &y = (obj.*br)();
  Derived *z = (obj.*dg)();
  Base *w = (bp->*bn)();
  return (x == bp) + 2 * (&y == bp) + 4 * (z == &obj) + 8 * (x->b == 1) +
         16 * (z->d == 2) + 32 * (w == bp) + 64 * (static_cast<Derived *>(x) == z);
}
static_assert(f() == 127);
