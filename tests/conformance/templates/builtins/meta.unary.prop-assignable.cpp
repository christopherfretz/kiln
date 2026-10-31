// [meta.unary.prop]/3 is_assignable: __is_assignable(T, U) is whether
// declval<T>() = declval<U>() is well-formed as an unevaluated operand
// (access checked outside any class; deleted, ambiguous and inaccessible
// assignment operators give false; cv void gives false).
//@ expect: compile-ok
//@ strict-diags
struct A {};
struct B { B& operator=(const B&) = delete; };
struct C { C& operator=(int); };
class P { P& operator=(const P&); friend void f(); };
struct M { M& operator=(M&&); M& operator=(const M&) = delete; };
struct Inc;
enum E { e0 };
enum class SE { x };
static_assert(__is_assignable(int&, int));
static_assert(__is_assignable(int&, double));
static_assert(!__is_assignable(int, int));
static_assert(!__is_assignable(int&&, int));
static_assert(!__is_assignable(const int&, int));
static_assert(__is_assignable(A, A));
static_assert(__is_assignable(A&, const A&));
static_assert(!__is_assignable(B&, const B&));
static_assert(__is_assignable(C&, int));
static_assert(!__is_assignable(C&, int*));
static_assert(!__is_assignable(P&, const P&));
static_assert(__is_assignable(M&, M));
static_assert(!__is_assignable(M&, M&));
static_assert(!__is_assignable(void, int));
static_assert(!__is_assignable(int&, void));
static_assert(__is_assignable(E&, E));
static_assert(!__is_assignable(E&, int));
static_assert(!__is_assignable(SE&, int));
static_assert(__is_assignable(int*&, decltype(nullptr)));
static_assert(!__is_assignable(int[2], int[2]));
static_assert(!__is_assignable(int(&)[2], int(&)[2]));
static_assert(__is_assignable(Inc*&, Inc*));
static_assert(!__is_assignable(void(&)(), void()));
template <class T, class U> constexpr bool ia = __is_assignable(T, U);
static_assert(ia<A&, A> && !ia<B&, B>);
template <class T, class U, bool = __is_assignable(T, U)> struct S { static constexpr bool v = false; };
template <class T, class U> struct S<T, U, true> { static constexpr bool v = true; };
static_assert(S<int&, long>::v && !S<const int&, long>::v);
#if !__has_builtin(__is_assignable)
#error no
#endif
int main() {}
