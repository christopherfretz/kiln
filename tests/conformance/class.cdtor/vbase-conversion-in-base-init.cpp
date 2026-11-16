// [class.cdtor]/3: a base initializer that uses `this` (here converting it to a virtual base)
// runs after the constructor has stored its vtable pointers from the VTT, as clang's
// `EmitBaseInitializer` does; they are stored again after the base initializers. Found running
// GCC's C++ testsuite.
// The IR is release-specific (the datalayout line and frame-pointer attributes
// differ between clang 19 and 22): pinned to clang 22 here and to clang 19 in its twin
// vbase-conversion-in-base-init-19.cpp (a clang compiler under test is never
// compared with the other release's oracle).
//@ expect: ir
//@ requires: clang>=22
//@ args: -fno-rtti
struct A { A(); };
struct B { B(A*); };
struct C : virtual A, B { C(); };
C::C() : B(this) {}
struct D : virtual C {};
D d;
