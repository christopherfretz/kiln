// [depr.impldec]/1: a user-declared destructor deprecates the implicit
// copy members. clang: -Wdeprecated-copy-with-dtor (in -Wdeprecated, not
// -Wextra) and -Wdeprecated-copy-with-user-provided-dtor.
//@ verify
//@ args: -Wdeprecated

struct B { ~B() {} }; // expected-warning {{definition of implicit copy constructor for 'B' is deprecated because it has a user-provided destructor}} expected-warning {{definition of implicit copy assignment operator for 'B' is deprecated because it has a user-provided destructor}}
struct D { ~D() = default; }; // expected-warning {{definition of implicit copy constructor for 'D' is deprecated because it has a user-declared destructor}}
void f(B b) {
  B b2 = b; // expected-note {{in implicit copy constructor for 'B' first required here}}
  b2 = b; // expected-note {{in implicit copy assignment operator for 'B' first required here}}
  D d; D d2 = d; // expected-note {{in implicit copy constructor for 'D' first required here}}
}
