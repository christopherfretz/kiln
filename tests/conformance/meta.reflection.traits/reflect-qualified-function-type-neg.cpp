// [dcl.fct]/6: outside the operand of `^^` (and the other permitted
// contexts), declaring a non-member function through a typedef-name for
// a cv-qualified function type is an error, with -freflection too.
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 12:1 error "non-member function of type 'FnC' (aka 'int () const') cannot have 'const' qualifier"
using FnC = int() const;
constexpr auto ok = ^^FnC;
FnC f;
