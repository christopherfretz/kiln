// [class.compare.secondary]/2: a defaulted secondary comparison operator
// is deleted if overload resolution for `x @ y` fails, or selects a
// candidate that is not a rewritten `<=>` (or `==`) candidate; its
// declared return type must be `bool` ([class.compare.default]/1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:8 warning "explicitly defaulted relational comparison operator is implicitly deleted"
//@ diag: 26:8 note "defaulted 'operator<' is implicitly deleted because there is no viable three-way comparison function for 'NoCmp'"
//@ diag: 26:40 note "replace 'default' with 'delete'"
//@ diag: 30:8 warning "explicitly defaulted relational comparison operator is implicitly deleted"
//@ diag: 31:15 note "defaulted 'operator<' is implicitly deleted because this non-rewritten comparison function would be the best match for the comparison"
//@ diag: 30:41 note "replace 'default' with 'delete'"
//@ diag: 35:8 warning "explicitly defaulted equality comparison operator is implicitly deleted"
//@ diag: 35:8 note "defaulted 'operator!=' is implicitly deleted because there is no viable 'operator==' for 'NoEq'"
//@ diag: 35:40 note "replace 'default' with 'delete'"
//@ diag: 40:7 error "return type for defaulted relational comparison operator must be 'bool', not 'int'"
//@ diag: 42:38 error "object of type 'NoCmp' cannot be compared because its 'operator<' is implicitly deleted"
//@ diag: 26:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 26:8 note "defaulted 'operator<' is implicitly deleted because there is no viable three-way comparison function for 'NoCmp'"
//@ diag: 42:47 error "object of type 'NoEq' cannot be compared because its 'operator!=' is implicitly deleted"
//@ diag: 35:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 35:8 note "defaulted 'operator!=' is implicitly deleted because there is no viable 'operator==' for 'NoEq'"
#include "../cmp.categories/Inputs/compare.h"
struct NoCmp {
  int x;
  bool operator<(const NoCmp&) const = default;
};
struct Direct {
  int x;
  bool operator<(const Direct&) const = default;
  friend bool operator<(const Direct&, const Direct&);
};
struct NoEq {
  int x;
  bool operator!=(const NoEq&) const = default;
};
struct BadRet {
  int x;
  auto operator<=>(const BadRet&) const = default;
  int operator<(const BadRet&) const = default;
};
bool use(NoCmp a, NoEq c) { return a < a || c != c; }
