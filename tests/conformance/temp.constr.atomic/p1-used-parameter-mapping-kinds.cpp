// [temp.constr.atomic]/1, [temp.constr.order]: an atomic constraint's
// parameter mapping covers only the template parameters its expression
// names, whatever kind of expression or type names them: a new- or
// delete-expression, a throw-expression, a pseudo-destructor call, a
// pack-indexing expression or type, a type trait over a transformed or
// typeof type, an array type trait, offsetof, and a template named
// through a using-declaration. `C<T, int>` and `C<T, long>` give the same
// atom, so the overload adding a conjunct is more constrained (an
// over-wide mapping would make each call ambiguous). WP-776.
//@ expect: compile-ok
struct S { int x; };
namespace ns { template <class> struct W {}; }
using ns::W;

template <class T, class U> concept New = requires { new T; };
template <class T> requires New<T, int> constexpr int f1(T) { return 1; }
template <class T> requires New<T, long> && true constexpr int f1(T) { return 2; }
static_assert(f1(1) == 2);

template <class T, class U> concept Del = requires(T* p) { delete p; p->~T(); };
template <class T> requires Del<T, int> constexpr int f2(T) { return 1; }
template <class T> requires Del<T, long> && true constexpr int f2(T) { return 2; }
static_assert(f2(S{}) == 2);

template <class T, class U> concept Throw = requires { throw T(); };
template <class T> requires Throw<T, int> constexpr int f3(T) { return 1; }
template <class T> requires Throw<T, long> && true constexpr int f3(T) { return 2; }
static_assert(f3(1) == 2);

template <class U, class... Ts> concept IdxT = sizeof(Ts...[0]) > 0;
template <class T> requires IdxT<int, T> constexpr int f4(T) { return 1; }
template <class T> requires IdxT<long, T> && true constexpr int f4(T) { return 2; }
static_assert(f4(1) == 2);

template <class U, auto... Ns> concept IdxE = Ns...[0] >= 0;
template <int N> requires IdxE<int, N> constexpr int f5() { return 1; }
template <int N> requires IdxE<long, N> && true constexpr int f5() { return 2; }
static_assert(f5<1>() == 2);

template <class T, class U> concept Xform = __is_same(__remove_cvref(T), T);
template <class T> requires Xform<T, int> constexpr int f6(T) { return 1; }
template <class T> requires Xform<T, long> && true constexpr int f6(T) { return 2; }
static_assert(f6(1) == 2);

template <class T, class U> concept TypeOf = sizeof(__typeof__(T{})) > 0;
template <class T> requires TypeOf<T, int> constexpr int f7(T) { return 1; }
template <class T> requires TypeOf<T, long> && true constexpr int f7(T) { return 2; }
static_assert(f7(1) == 2);

template <class T, class U> concept Rank = __array_rank(T) == 0;
template <class T> requires Rank<T, int> constexpr int f8(T) { return 1; }
template <class T> requires Rank<T, long> && true constexpr int f8(T) { return 2; }
static_assert(f8(1) == 2);

template <class T, class U> concept Off = __builtin_offsetof(T, x) == 0;
template <class T> requires Off<T, int> constexpr int f9(T) { return 1; }
template <class T> requires Off<T, long> && true constexpr int f9(T) { return 2; }
static_assert(f9(S{}) == 2);

template <class T, class U> concept Using = sizeof(W<T>) > 0;
template <class T> requires Using<T, int> constexpr int f10(T) { return 1; }
template <class T> requires Using<T, long> && true constexpr int f10(T) { return 2; }
static_assert(f10(1) == 2);
