// [temp.constr.atomic]/2: atomic constraints are identical when their
// parameter mappings' targets are equivalent, and a non-dependent argument
// of a non-integral non-type parameter maps as its converted template
// argument ([temp.arg.nontype]/2), compared by template-argument-
// equivalence ([temp.type]/2): `&x` and `&(x)`, `S{1}` and `S{0 + 1}`,
// `x` and `(x)` for a reference, `1` and `2 - 1` for `auto` give one atom,
// so the overload adding a conjunct is more constrained
// ([temp.constr.order]). Different values are the `-neg` sibling. clang 19
// and 22 agree. WP-821.
//@ expect: compile-ok
//@ strict-diags
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
