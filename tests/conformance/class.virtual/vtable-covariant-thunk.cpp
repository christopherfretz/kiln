// [class.virtual]/8-9 (covariant return types), Itanium C++ ABI §2.5.2,
// §2.6 (a covariant overrider whose return needs a pointer adjustment gets a
// return-adjusting thunk in the base's slot and a new slot of its own).
//@ expect: vtables
struct X { virtual void x(); int i; };
struct Base { virtual Base *clone(); virtual X *make(); };
struct Y { virtual void y(); int pad; };
struct Z : Y, X { void x() override; };   // Y primary, X at offset 16 in Z
struct Derived : Base {
  Derived *clone() override;              // no adjustment: Base is primary
  Z *make() override;                     // return adjusted by +16
};
void X::x() {} void Y::y() {} void Z::x() {}
Base *Base::clone() { return this; } X *Base::make() { return nullptr; }
Derived *Derived::clone() { return this; } Z *Derived::make() { return nullptr; }
