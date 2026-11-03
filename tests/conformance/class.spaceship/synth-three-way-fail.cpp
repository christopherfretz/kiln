// [class.spaceship]/1: the synthesized three-way comparison needs a
// usable `<` whose result converts to `bool` (clang diagnoses the
// conversion in the implicit definition) and a declared comparison
// category: with a deduced return type (`auto`), a subobject whose
// `operator<=>` return type is itself still undeduced makes the
// defaulted `<=>` ill-formed and deleted ([dcl.spec.auto]/11).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:8 error "return type of defaulted 'operator<=>' cannot be deduced because three-way comparison for member 'j' has a deduced return type and is not yet defined"
//@ diag: 31:5 note "member 'j' declared here"
//@ diag: 29:24 note "selected 'operator<=>' for member 'j' declared here"
//@ diag: 32:8 warning "explicitly defaulted three-way comparison operator is implicitly deleted"
//@ diag: 32:41 note "replace 'default' with 'delete'"
//@ diag: 35:44 warning "explicitly defaulted three-way comparison operator is implicitly deleted"
//@ diag: 35:22 note "defaulted 'operator<=>' is implicitly deleted because there is no viable three-way comparison function for member 'l'"
//@ diag: 35:22 note "three-way comparison cannot be synthesized because there is no viable function for '<' comparison"
//@ diag: 35:77 note "replace 'default' with 'delete'"
//@ diag: 36:43 error "object of type 'UseJ' cannot be compared because its 'operator<=>' is implicitly deleted"
//@ diag: 32:8 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 28:85 error "value of type 'void' is not contextually convertible to 'bool'"
//@ diag: 36:60 note "in defaulted three-way comparison operator for 'UseB' first required here"
//@ diag: 36:77 error "object of type 'UseN' cannot be compared because its 'operator<=>' is implicitly deleted"
//@ diag: 35:44 note "explicitly defaulted function was implicitly deleted here"
//@ diag: 35:22 note "defaulted 'operator<=>' is implicitly deleted because there is no viable three-way comparison function for member 'l'"
//@ diag: 35:22 note "three-way comparison cannot be synthesized because there is no viable function for '<' comparison"
#include "../cmp.categories/Inputs/compare.h"
struct BadRet { int n; bool operator==(const BadRet&) const = default; void operator<(const BadRet&) const; };
struct UseB { BadRet l; std::strong_ordering operator<=>(const UseB&) const = default; };
struct J { int n; auto operator<=>(const J&) const; bool operator==(const J&) const; bool operator<(const J&) const; };
struct UseJ {
  J j;
  auto operator<=>(const UseJ&) const = default;
};
struct NoLess { int n; bool operator==(const NoLess&) const; };
struct UseN { NoLess l; std::weak_ordering operator<=>(const UseN&) const = default; };
void f(UseB b, UseJ j, UseN n) { (void)(j <=> j); (void)(b <=> b); (void)(n <=> n); }
