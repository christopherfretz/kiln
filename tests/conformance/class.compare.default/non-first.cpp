// [dcl.fct.def.default]/5, [class.compare.default]/1: a comparison may be
// defaulted outside the class only if declared in it; defaulting in the
// class after an earlier declaration outside it is ill-formed. (A constexpr
// out-of-class default after a non-constexpr declaration is a general
// redeclaration gap, spec/sema-class.md §11.)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:15 error "defaulting this equality comparison operator is not allowed because it was already declared outside the class"
//@ diag: 14:6 note "previous declaration is here"
//@ diag: 23:9 error "defaulting this equality comparison operator would delete it after its first declaration"
//@ diag: 20:8 note "defaulted 'operator==' is implicitly deleted because class 'B' has a reference member"
#include "../cmp.categories/Inputs/compare.h"
struct A;
bool operator==(const A&, const A&);
struct A {
  int x;
  friend bool operator==(const A&, const A&) = default;
};
struct B {
  int& r;
  bool operator==(const B&) const;
};
bool B::operator==(const B&) const = default;
