// [class.cdtor]/3: a base initializer that uses `this` (here converting it to a virtual base)
// runs after the constructor has stored its vtable pointers from the VTT, as clang's
// `EmitBaseInitializer` does; they are stored again after the base initializers. Found running
// GCC's C++ testsuite.
//@ expect: ir
//@ args: -fno-rtti
struct A { A(); };
struct B { B(A*); };
struct C : virtual A, B { C(); };
C::C() : B(this) {}
struct D : virtual C {};
D d;
