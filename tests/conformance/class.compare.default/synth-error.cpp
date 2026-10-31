// [class.compare.default]/6, [class.eq]/3: an error in the implicit
// definition is reported with the synthesis context note.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:43 error "value of type 'NotBool' is not contextually convertible to 'bool'"
//@ diag: 15:24 note "in defaulted equality comparison operator for 'S' first required here"
#include "../cmp.categories/Inputs/compare.h"
struct X {};
struct NotBool { bool v; };
NotBool operator==(const X&, const X&);
struct S {
  X x;
  bool operator==(const S&) const = default;
};
bool f(S a) { return a == a; }
