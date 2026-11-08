// [depr.impldec]/1: -Werror=deprecated-copy makes the user-declared and
// (through its subgroup) the user-provided flavour errors; it is off by
// default.
//@ verify
//@ args: -Werror=deprecated-copy

struct C { C(const C &) = default; C() = default; }; // expected-error {{definition of implicit copy assignment operator for 'C' is deprecated because it has a user-declared copy constructor}}
struct E { E(const E&) {} E() {} }; // expected-error {{definition of implicit copy assignment operator for 'E' is deprecated because it has a user-provided copy constructor}}
void f() {
  C c; C c2; c2 = c; // expected-note {{in implicit copy assignment operator for 'C' first required here}}
  E e; E e2; e2 = e; // expected-note {{in implicit copy assignment operator for 'E' first required here}}
}
