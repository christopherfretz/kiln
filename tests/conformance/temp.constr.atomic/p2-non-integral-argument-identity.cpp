// [temp.constr.atomic]/2: atomic constraints are identical when their
// parameter mappings' targets are equivalent, and a non-dependent argument
// of a non-integral non-type parameter maps as its converted template
// argument ([temp.arg.nontype]/2), compared by template-argument-
// equivalence ([temp.type]/2): `&x` and `&(x)`, `S{1}` and `S{0 + 1}`,
// `x` and `(x)` for a reference, `1` and `2 - 1` for `auto` give one atom,
// so the overload adding a conjunct is more constrained
// ([temp.constr.order]). Different values (`&x` and `&y`, `S{1}` and
// `S{2}`) give two atoms and the call is ambiguous. WP-821.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 51:15 error "call to 'np' is ambiguous"
//@ diag: 49:53 note "candidate function [with T = int]"
//@ diag: 50:61 note "candidate function [with T = int]"
//@ diag: 56:15 error "call to 'ns' is ambiguous"
//@ diag: 54:55 note "candidate function [with T = int]"
//@ diag: 55:63 note "candidate function [with T = int]"
struct S { int v; };
int x, y;

template <class T, const int* P> concept CP = sizeof(T) > 0 && P != nullptr;
template <class T> requires CP<T, &x> constexpr int f(T) { return 1; }
template <class T> requires CP<T, &(x)> && true constexpr int f(T) { return 2; }
static_assert(f(0) == 2);

template <class T, S V> concept CS = sizeof(T) > 0 && V.v > 0;
template <class T> requires CS<T, S{1}> constexpr int g(T) { return 1; }
template <class T> requires CS<T, S{0 + 1}> && true constexpr int g(T) { return 2; }
static_assert(g(0) == 2);

template <class T, const int& R> concept CR = sizeof(T) > 0 && sizeof(R) > 0;
template <class T> requires CR<T, x> constexpr int h(T) { return 1; }
template <class T> requires CR<T, (x)> && true constexpr int h(T) { return 2; }
static_assert(h(0) == 2);

template <class T, auto V> concept CA = sizeof(T) > 0 && V != 0;
template <class T> requires CA<T, 1> constexpr int k(T) { return 1; }
template <class T> requires CA<T, 2 - 1> && true constexpr int k(T) { return 2; }
static_assert(k(0) == 2);

// The parameter's type depends on an earlier parameter.
template <class T, class U, U V> concept CT = sizeof(T) > 0 && V != nullptr;
template <class T> requires CT<T, const int*, &x> constexpr int m(T) { return 1; }
template <class T> requires CT<T, const int*, &(x)> && true constexpr int m(T) { return 2; }
static_assert(m(0) == 2);

// Different values: two atoms.
template <class T, const int* P> concept DP = sizeof(T) > 0 && P != nullptr;
template <class T> requires DP<T, &x> constexpr int np(T) { return 1; }
template <class T> requires DP<T, &y> && true constexpr int np(T) { return 2; }
static_assert(np(0) == 2);

template <class T, S V> concept DS = sizeof(T) > 0 && V.v > 0;
template <class T> requires DS<T, S{1}> constexpr int ns(T) { return 1; }
template <class T> requires DS<T, S{2}> && true constexpr int ns(T) { return 2; }
static_assert(ns(0) == 2);
