// [class.compare.default]/1-2, [dcl.fct.def.default]: the declaration
// checks on an explicitly defaulted comparison operator function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:56 error "comparison operator template cannot be defaulted"
//@ diag: 18:8 error "defaulted member equality comparison operator must be const-qualified"
//@ diag: 23:14 error "deduced return type for defaulted three-way comparison operator must be 'auto', not 'const auto'"
//@ diag: 19:8 error "ref-qualifier '&&' is not allowed on a defaulted comparison operator"
//@ diag: 20:8 error "defaulted comparison function must not be volatile"
//@ diag: 21:8 error "invalid parameter type for defaulted relational comparison operator; found 'A', expected 'const A &'"
//@ diag: 22:7 error "return type for defaulted relational comparison operator must be 'bool', not 'int'"
//@ diag: 27:6 error "equality comparison operator is not a friend of incomplete class 'B'"
//@ diag: 26:8 note "forward declaration of 'B'"
//@ diag: 30:15 error "overloaded 'operator!=' must have at least one parameter of class or enumeration type"
//@ diag: 29:15 error "parameters for defaulted equality comparison operator must have the same type (found 'const C &' vs 'C')"
#include "../cmp.categories/Inputs/compare.h"
struct A {
  bool operator==(const A&) = default;
  bool operator<(const A&) const&& = default;
  bool operator!=(const A&) const volatile = default;
  bool operator<=(A) const = default;
  int operator>(const A&) const = default;
  const auto operator<=>(const A&) const = default;
  template <class T> bool operator>=(const T&) const = default;
};
struct B;
bool operator==(const B&, const B&) = default;
struct C {
  friend bool operator==(const C&, C) = default;
  friend bool operator!=(int, int) = default;
};
struct D {
  bool operator==(const D&) const;
};
bool D::operator==(const D&) const = default;
