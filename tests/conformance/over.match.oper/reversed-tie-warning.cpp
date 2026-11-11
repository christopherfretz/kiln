// [over.match.oper]/3.4, [over.match.best.general]/2: outside a SFINAE
// context a tie between a non-reversed candidate and the reversed form of
// a candidate is accepted with clang's extension warning
// (-Wambiguous-reversed-operator), for `<`/`<=` rewritten from `<=>` and
// for `==`/`!=` against a pre-C++20 `operator==(const Base&)`; GCC 16.1
// accepts too, with its own warning ("C++20 says that these are
// ambiguous, even though the second is reversed"). The same use in a
// SFINAE context is a substitution failure (reversed-tie-sfinae-run.cpp,
// WP-1322).
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ diag: 24:56 warning "ISO C++20 considers use of overloaded operator '<' (with operand types 'const Derived' and 'const Derived') to be ambiguous despite there being a unique best viable function"
//@ diag: 20:24 note "ambiguity is between a regular call to this operator and a call with the argument order reversed"
//@ diag: 25:48 warning "ISO C++20 considers use of overloaded operator '!=' (with operand types 'const EqD' and 'const EqD') to be ambiguous despite there being a unique best viable function"
//@ diag: 23:25 note "ambiguity is between a regular call to this operator and a call with the argument order reversed"
#include <compare>
struct Base { int v = 0; };
struct Derived : Base {
  std::strong_ordering operator<=>(const Base &o) const { return v <=> o.v; }
};
struct EqB { int v = 0; };
struct EqD : EqB { bool operator==(const EqB &o) const { return v == o.v; } };
bool lt(const Derived &a, const Derived &b) { return a < b; }
bool ne(const EqD &a, const EqD &b) { return a != b; }
