// [class.compare.default]/2, [class.eq]/2, [class.spaceship]/2: a
// defaulted comparison is implicitly deleted when a subobject comparison
// is not usable; using it is an error with the analyzer's notes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:8 warning "explicitly defaulted equality comparison operator is implicitly deleted"
//@ diag: 43:8 note "defaulted 'operator==' is implicitly deleted because class 'HasRef' has a reference member"
//@ diag: 44:42 note "replace 'default' with 'delete'"
//@ diag: 48:8 warning "explicitly defaulted equality comparison operator is implicitly deleted"
//@ diag: 48:8 note "defaulted 'operator==' is implicitly deleted because 'U' is a union with variant members"
//@ diag: 48:37 note "replace 'default' with 'delete'"
//@ diag: 52:8 warning "explicitly defaulted equality comparison operator is implicitly deleted"
//@ diag: 51:8 note "defaulted 'operator==' is implicitly deleted because there is no viable 'operator==' for member 'n'"
//@ diag: 52:37 note "replace 'default' with 'delete'"
//@ diag: 58:8 warning "explicitly defaulted equality comparison operator is implicitly deleted"
//@ diag: 57:15 note "defaulted 'operator==' is implicitly deleted because it would invoke a deleted comparison function for base class 'Del'"
//@ diag: 55:8 note "'operator==' has been explicitly marked deleted here"
//@ diag: 58:40 note "replace 'default' with 'delete'"
//@ diag: 65:8 warning "explicitly defaulted equality comparison operator is implicitly deleted"
//@ diag: 64:8 note "defaulted 'operator==' is implicitly deleted because it would invoke a private 'operator==' member of 'Priv' to compare member 'p'"
//@ diag: 61:8 note "implicitly declared private here"
//@ diag: 65:44 note "replace 'default' with 'delete'"
//@ diag: 68:12 error "object of type 'HasRef' cannot be compared because its 'operator==' is implicitly deleted"
//@ diag: 44:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 43:8 note "defaulted 'operator==' is implicitly deleted because class 'HasRef' has a reference member"
//@ diag: 68:22 error "object of type 'U' cannot be compared because its 'operator==' is implicitly deleted"
//@ diag: 48:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 48:8 note "defaulted 'operator==' is implicitly deleted because 'U' is a union with variant members"
//@ diag: 68:32 error "object of type 'M' cannot be compared because its 'operator==' is implicitly deleted"
//@ diag: 52:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 51:8 note "defaulted 'operator==' is implicitly deleted because there is no viable 'operator==' for member 'n'"
//@ diag: 68:42 error "object of type 'Base' cannot be compared because its 'operator==' is implicitly deleted"
//@ diag: 58:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 57:15 note "defaulted 'operator==' is implicitly deleted because it would invoke a deleted comparison function for base class 'Del'"
//@ diag: 55:8 note "'operator==' has been explicitly marked deleted here"
//@ diag: 68:52 error "object of type 'UsesPriv' cannot be compared because its 'operator==' is implicitly deleted"
//@ diag: 65:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 64:8 note "defaulted 'operator==' is implicitly deleted because it would invoke a private 'operator==' member of 'Priv' to compare member 'p'"
//@ diag: 61:8 note "implicitly declared private here"
#include "../cmp.categories/Inputs/compare.h"
struct NoEq {};
struct HasRef {
  int& r;
  bool operator==(const HasRef&) const = default;
};
union U {
  int i;
  bool operator==(const U&) const = default;
};
struct M {
  NoEq n;
  bool operator==(const M&) const = default;
};
struct Del {
  bool operator==(const Del&) const = delete;
};
struct Base : Del {
  bool operator==(const Base&) const = default;
};
class Priv {
  bool operator==(const Priv&) const;
};
struct UsesPriv {
  Priv p;
  bool operator==(const UsesPriv&) const = default;
};
bool f(HasRef a, U u, M m, Base b, UsesPriv p) {
  return a == a || u == u || m == m || b == b || p == p;
}
