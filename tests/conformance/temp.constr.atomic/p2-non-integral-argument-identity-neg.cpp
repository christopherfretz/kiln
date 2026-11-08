// [temp.constr.atomic]/2: different converted values of a non-integral
// non-type parameter (`&x` and `&y`, `S{1}` and `S{2}`) give two atoms
// (the template-ids `A<&x>` and `A<&y>` are different types), so neither
// overload subsumes the other and the call is ambiguous
// ([temp.constr.order]). See p2-non-integral-argument-identity.cpp. WP-821.
// GCC 16.1 and clang 19 agree; clang 22 accepts and picks the overload
// adding a conjunct, even for `C<T, 1>` against `C<T, 2>` with an `int`
// parameter (its P2841 normalization rework; not the wording).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 treats atoms whose mappings differ only in a non-type argument of a dependent concept-id as identical; GCC 16.1 and clang 19 report the ambiguity
//@ oracle: gcc
//@ diag: 24:15 error "call to 'np' is ambiguous"
//@ diag: 22:53 note "candidate function [with T = int]"
//@ diag: 23:61 note "candidate function [with T = int]"
//@ diag: 29:15 error "call to 'ns' is ambiguous"
//@ diag: 27:55 note "candidate function [with T = int]"
//@ diag: 28:63 note "candidate function [with T = int]"
struct S { int v; };
int x, y;
template <class T, const int* P> concept DP = sizeof(T) > 0 && P != nullptr;
template <class T> requires DP<T, &x> constexpr int np(T) { return 1; }
template <class T> requires DP<T, &y> && true constexpr int np(T) { return 2; }
static_assert(np(0) == 2);

template <class T, S V> concept DS = sizeof(T) > 0 && V.v > 0;
template <class T> requires DS<T, S{1}> constexpr int ns(T) { return 1; }
template <class T> requires DS<T, S{2}> && true constexpr int ns(T) { return 2; }
static_assert(ns(0) == 2);
