// [class.compare.default]/4: the implicit `operator==` of a defaulted
// `operator<=>` has the same trailing requires-clause; in a class
// template specialization it is instantiated from the pattern's implicit
// declaration, so its constraints are checked with the specialization's
// arguments, and the candidate note names it "the implicit 'operator=='"
// (member or friend). [dcl.fct.def.default]/5 via clang
// `MergeFunctionDecl`: an implicit `==` (or an explicitly defaulted
// member) is not defined again, ahead of the exception-specification
// and constexpr checks.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:19 error "invalid operands to binary expression ('A<int>' and 'A<int>')"
//@ diag: 30:8 note "candidate function (the implicit 'operator==' for this 'operator<=>)' not viable: constraints not satisfied"
//@ diag: 30:46 note "because 'sizeof(int) > 100' (4 > 100) evaluated to false"
//@ diag: 30:8 note "candidate function (the implicit 'operator==' for this 'operator<=>)' not viable: constraints not satisfied"
//@ diag: 30:46 note "because 'sizeof(int) > 100' (4 > 100) evaluated to false"
//@ diag: 37:19 error "invalid operands to binary expression ('C<int>' and 'C<int>')"
//@ diag: 35:15 note "candidate function (the implicit 'operator==' for this 'operator<=>)' not viable: constraints not satisfied"
//@ diag: 35:57 note "because 'sizeof(int) > 100' (4 > 100) evaluated to false"
//@ diag: 43:26 error "definition of implicitly declared function"
//@ diag: 45:9 error "definition of implicitly declared function"
//@ diag: 47:9 error "definition of explicitly defaulted function"
//@ diag: 49:4 error "definition of explicitly defaulted default constructor"
//@ diag: 50:4 error "definition of explicitly defaulted destructor"
//@ diag: 52:29 error "invalid operands to binary expression ('E' and 'B')"
//@ diag: 44:24 note "candidate function (the implicit 'operator==' for this 'operator<=>)' not viable: no known conversion from 'B' to 'const E' for 1st argument"
#include "../cmp.categories/Inputs/compare.h"
template<class T> struct A {
  int n;
  auto operator<=>(const A&) const requires (sizeof(T) > 100) = default;
};
bool a = A<int>{} == A<int>{};
template<class T> struct C {
  int n;
  friend auto operator<=>(const C&, const C&) requires (sizeof(T) > 100) = default;
};
bool c = C<int>{} == C<int>{};
template<class T> struct D {
  T n;
  constexpr auto operator<=>(const D&) const = default;
};
static_assert(D<int>{1} == D<int>{1} && D<char>{1} != D<char>{2});
template<> bool D<long>::operator==(const D<long>&) const { return false; }
struct E { int n; auto operator<=>(const E&) const = default; };
bool E::operator==(const E&) const { return false; }
struct G { int n; bool operator==(const G&) const = default; };
bool G::operator==(const G&) const { return true; }
struct H { H() = default; ~H() = default; };
H::H() {}
H::~H() {}
struct B { int n; };
void f(E e, B b) { (void)(e == b); }
