// [temp.constr.atomic]/2: atomic constraints are identical when their
// parameter mappings' targets are equivalent. A non-dependent argument
// of a concept-id in a template's requires-clause maps as its converted
// value ([temp.arg.nontype]/1), so `C<1, int>` and `C<2 - 1, long>` give
// one atom `N > 0` (U is unused) and the overload adding a conjunct is
// more constrained, while `C<1, int>` and `C<2, long>` give two atoms
// and the call is ambiguous. WP-776.
//@ expect: compile-ok
template <int N, class U> concept C = N > 0;
enum E { e0, e1 };
template <E V> concept D = V != e0;

template <class T> requires C<1, int> constexpr int f1(T) { return 1; }
template <class T> requires C<1, long> && true constexpr int f1(T) { return 2; }
static_assert(f1(0) == 2);

template <class T> requires C<1, int> constexpr int f2(T) { return 1; }
template <class T> requires C<2 - 1, long> && true constexpr int f2(T) { return 2; }
static_assert(f2(0) == 2);

template <class T> requires D<e1> constexpr int f3(T) { return 1; }
template <class T> requires D<E(1)> && true constexpr int f3(T) { return 2; }
static_assert(f3(0) == 2);

template <class T> requires C<1, int> constexpr int f4(T) { return 1; }
template <class T> requires C<2, long> && true constexpr int f4(T) { return 2; }
template <class T> concept CallsF4 = requires (T t) { f4(t); };
static_assert(!CallsF4<int>);
