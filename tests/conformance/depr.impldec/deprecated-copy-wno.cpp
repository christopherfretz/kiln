// [depr.impldec]/1: -Wno-deprecated-copy-with-user-provided-copy keeps
// the user-declared flavour under -Wextra and drops the user-provided one.
//@ verify
//@ args: -Wextra -Wno-deprecated-copy-with-user-provided-copy

struct C { C(const C &) = default; C() = default; }; // expected-warning {{definition of implicit copy assignment operator for 'C' is deprecated because it has a user-declared copy constructor}}
struct E { E(const E&) {} E() {} };
void f() {
  C c; C c2; c2 = c; // expected-note {{in implicit copy assignment operator for 'C' first required here}}
  E e; E e2; e2 = e;
}
