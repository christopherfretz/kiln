// [temp.constr.atomic]/1: an atomic constraint's parameter mapping covers
// only the template parameters its expression names, so `sizeof(T) > 1` of
// C<T, int> and of C<T, long> is the same atomic constraint and the second
// overload, which adds a conjunct, subsumes the first ([temp.constr.order]).
// Type, non-type and template template parameters the atom does not name,
// one named only through a requires-expression, a nested-name-specifier or a
// type trait, and an unused parameter mapped to a dependent argument; a
// parameter the atom does name still tells the atoms apart (ambiguous).
// WP-771.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 59:9 error "call to 'p' is ambiguous"
//@ diag: 57:54 note "candidate function [with T = int]"
//@ diag: 58:63 note "candidate function [with T = int]"
// type parameter unused by the atom
template <class T, class U> concept C = sizeof(T) > 1;
template <class T> requires C<T, int> constexpr int f(T) { return 1; }
template <class T> requires C<T, long> && true constexpr int f(T) { return 2; }
static_assert(f(1L) == 2);

// non-type parameter unused
template <class T, int N> concept D = __is_integral(T);
template <class T> requires D<T, 1> constexpr int g(T) { return 1; }
template <class T> requires D<T, 2> && (sizeof(T) > 0) constexpr int g(T) { return 2; }
static_assert(g(1) == 2);

// template template parameter unused
template <class> struct X {};
template <class> struct Y {};
template <class T, template <class> class Tt> concept E = __is_class(T);
template <class T> requires E<T, X> constexpr int h(T) { return 1; }
template <class T> requires E<T, Y> && (sizeof(T) > 0) constexpr int h(T) { return 2; }
struct S {};
static_assert(h(S{}) == 2);

// a requires-expression naming only T
template <class T, class U> concept F = requires(T t) { t + t; typename T::type; };
struct R { using type = int; friend R operator+(R, R); };
template <class T> requires F<T, char> constexpr int k(T) { return 1; }
template <class T> requires F<T, short> && true constexpr int k(T) { return 2; }
static_assert(k(R{}) == 2);

// the unused parameter mapped to a dependent argument
template <class T, class U> requires C<T, U*> constexpr int m(T, U) { return 1; }
template <class T, class U> requires C<T, U&> && true constexpr int m(T, U) { return 2; }
static_assert(m(1L, 0) == 2);

// a parameter named only through a nested-name-specifier or a type trait
template <class T, class U> concept G = T::value > 0 && __is_same(T, T);
struct V { static constexpr int value = 1; };
template <class T> requires G<T, int> constexpr int n(T) { return 1; }
template <class T> requires G<T, void> && true constexpr int n(T) { return 2; }
static_assert(n(V{}) == 2);

// a used parameter still distinguishes the atoms
template <class T, class U> concept H = sizeof(T) >= sizeof(U);
template <class T> requires H<T, char> constexpr int p(T) { return 1; }
template <class T> requires H<T, short> && true constexpr int p(T) { return 2; }
int q = p(1); // ambiguous
