// [depr.impldec]/1: the implicit definition of a copy constructor
// (assignment) is deprecated when the class has a user-declared copy
// assignment (constructor) or destructor. clang: -Wdeprecated-copy (in
// -Wextra) for a user-declared, defaulted member, and
// -Wdeprecated-copy-with-user-provided-copy for a user-provided one; the
// destructor flavours stay off under -Wextra. Each warns once, at the
// member, with a note where the definition was first required.
//@ verify
//@ args: -Wextra

struct A { A &operator=(const A &) { return *this; } }; // expected-warning {{definition of implicit copy constructor for 'A' is deprecated because it has a user-provided copy assignment operator}}
struct B { ~B() {} };
struct C { C(const C &) = default; C() = default; }; // expected-warning {{definition of implicit copy assignment operator for 'C' is deprecated because it has a user-declared copy constructor}}
struct D { ~D() = default; };
struct E { E(const E&) {} E() {} }; // expected-warning {{definition of implicit copy assignment operator for 'E' is deprecated because it has a user-provided copy constructor}}
void f() {
  A a; A a2 = a; // expected-note {{in implicit copy constructor for 'A' first required here}}
  B b; B b2 = b; b2 = b;
  C c; C c2; c2 = c; // expected-note {{in implicit copy assignment operator for 'C' first required here}}
  D d; D d2 = d;
  E e; E e2; e2 = e; // expected-note {{in implicit copy assignment operator for 'E' first required here}}
  A a3 = a;
}
