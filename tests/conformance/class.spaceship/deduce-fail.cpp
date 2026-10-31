// [class.spaceship]/2: a defaulted `auto <=>` whose subobject comparison
// does not yield a comparison category type, or has no usable `<=>`, is
// deleted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:8 warning "explicitly defaulted three-way comparison operator is implicitly deleted"
//@ diag: 23:10 note "return type of defaulted 'operator<=>' cannot be deduced because return type 'int' of three-way comparison for member 'm' is not a standard comparison category type"
//@ diag: 21:21 note "selected 'operator<=>' for member 'm' declared here"
//@ diag: 24:38 note "replace 'default' with 'delete'"
//@ diag: 29:8 warning "explicitly defaulted three-way comparison operator is implicitly deleted"
//@ diag: 28:9 note "defaulted 'operator<=>' is implicitly deleted because there is no viable three-way comparison function for member 'm'"
//@ diag: 29:38 note "replace 'default' with 'delete'"
//@ diag: 31:32 error "object of type 'A' cannot be compared because its 'operator<=>' is implicitly deleted"
//@ diag: 24:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 23:10 note "return type of defaulted 'operator<=>' cannot be deduced because return type 'int' of three-way comparison for member 'm' is not a standard comparison category type"
//@ diag: 21:21 note "selected 'operator<=>' for member 'm' declared here"
//@ diag: 31:50 error "object of type 'B' cannot be compared because its 'operator<=>' is implicitly deleted"
//@ diag: 29:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 28:9 note "defaulted 'operator<=>' is implicitly deleted because there is no viable three-way comparison function for member 'm'"
#include "../cmp.categories/Inputs/compare.h"
struct IntCmp { int operator<=>(const IntCmp&) const; bool operator==(const IntCmp&) const; };
struct A {
  IntCmp m;
  auto operator<=>(const A&) const = default;
};
struct NoCmp { bool operator==(const NoCmp&) const; };
struct B {
  NoCmp m;
  auto operator<=>(const B&) const = default;
};
bool use(A a, B b) { return (a <=> a) == 0 || (b <=> b) == 0; }
