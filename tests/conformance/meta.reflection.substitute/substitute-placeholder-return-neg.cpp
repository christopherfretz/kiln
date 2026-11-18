// [meta.reflection.substitute]/2-4, [dcl.spec.auto.general]/12,
// [dcl.type.auto.deduct]/2 (P2996R13): the definition substitute
// instantiates to deduce a placeholder return type is outside the
// substitution's immediate context, so a deduction that fails there
// (inconsistent return statements, a deduced type that does not satisfy
// the type-constraint) makes the program ill-formed, under can_substitute
// as under substitute, with the instantiation note at the call. A
// specialization with no reachable definition is not substitutable
// (substitute throws). GCC 16.1 does not instantiate a namespace-scope
// template here and accepts the first two (WP-1674).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 does not instantiate a namespace-scope function template to deduce its return type in substitute (WP-1674)
//@ strict-diags
//@ diag: 31:55 error "'auto' in return type deduced as 'double' here but deduced as 'int' in earlier return statement"
//@ diag: 32:21 note "in instantiation of function template specialization 'bad<int>' requested here"
//@ diag: 32:21 note "while substituting template arguments into 'bad' for std::meta::substitute"
//@ diag: 33:19 error "deduced type 'long' does not satisfy 'small'"
//@ diag: 34:21 note "in instantiation of function template specialization 'ci<long>' requested here"
//@ diag: 34:21 note "while substituting template arguments into 'ci' for std::meta::substitute"
//@ diag: 29:35 note "because 'sizeof(long) <= 2' (8 <= 2) evaluated to false"
//@ diag: 36:16 error "constexpr variable 'r1' must be initialized by a constant expression"
//@ diag: 36:21 note "'^^fn1' cannot be substituted with these template arguments: the specialization's type contains an undeduced placeholder type"
//@ diag: 36:21 error "call to consteval function 'std::meta::substitute<initializer_list<info>>' is not a constant expression"
//@ diag: 36:21 note "'^^fn1' cannot be substituted with these template arguments: the specialization's type contains an undeduced placeholder type"
#include <meta>
using namespace std::meta;
template<class T> concept small = sizeof(T) <= 2;

template<class T> auto bad(bool b) { if (b) return 1; return 1.0; }
constexpr bool b1 = can_substitute(^^bad, {^^int});
template<class T> small auto ci() { return T{}; }
constexpr info b2 = substitute(^^ci, {^^long});
template<class T> auto fn1();
constexpr info r1 = substitute(^^fn1, {^^int});
int main() {}
